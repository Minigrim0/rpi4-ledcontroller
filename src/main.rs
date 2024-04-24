use log::{info, warn, error};
use rs_ws281x::{ControllerBuilder, ChannelBuilder, StripType};
use rumqttc::{MqttOptions, Client, QoS};
use std::collections::HashMap;
use std::env;
use std::{thread, time};
use std::sync::{Arc, Mutex};

use crate::animation::Animation;

pub mod color;
pub mod animation;
pub mod rainbow;
pub mod off;
pub mod chase;

fn main(){
    env_logger::init();
    const WHEEL_LENGTH: i32 = 78;
    const STRIP_LENGTH: i32 = 96;

    // Animation factory type
    type AnimationFactory = Arc<dyn Fn() -> Box<dyn Animation>>;

    let mut animation_factories: HashMap<String, AnimationFactory> = HashMap::new();
    animation_factories.insert("rainbow".to_string(), Arc::new(|| Box::new(rainbow::Rainbow::new(STRIP_LENGTH, WHEEL_LENGTH))));
    animation_factories.insert("off".to_string(), Arc::new(|| Box::new(off::Off::new())));
    animation_factories.insert("chase".to_string(), Arc::new(|| Box::new(chase::Chase::new(WHEEL_LENGTH))));

    // Animation holder
    let mut current_animation: Box<dyn Animation> = animation_factories.get("off").unwrap()();

    let next_animation_name: Arc<Mutex<String>> = Arc::new(Mutex::new("".to_string()));
    let next_animation_name_clone = Arc::clone(&next_animation_name);

    thread::spawn(move || {
        let hostname = match env::args().next() {
            Some(h) => h,
            None => {
                warn!("No hostname set, defaulting to localhost");
                "localhost".to_string()
            }
        };

        let mut mqttoptions = MqttOptions::new("3d_leds", hostname, 1883);
        mqttoptions.set_keep_alive(time::Duration::new(60, 0));

        let (mut client, mut connection) = Client::new(mqttoptions, 10);
        info!("Connecting to MQTT broker...");
        client.subscribe("home/leds", QoS::AtLeastOnce).unwrap();
        let mut successive_failures: i8 = 0;

        for notification in connection.iter() {
            match notification {
                Ok(event) => if let rumqttc::Event::Incoming(rumqttc::Packet::Publish(p)) = event {
                    successive_failures = 0;  // Reset successive failures
                    if let Ok(s) = std::str::from_utf8(&p.payload) {
                        let mut next_animation_name = match next_animation_name_clone.lock() {
                            Ok(n) => n,
                            Err(e) => {
                                warn!("Unable to lock next_animation_name: {}", e);
                                continue;
                            }
                        };
                        *next_animation_name = s.to_string();
                    }
                },
                Err(error) => {
                    successive_failures += 1;
                    if successive_failures > 5 {
                        panic!("{} Connection errors\n{}\nExiting...", successive_failures, error.to_string());
                    }
                    let sleep_time = time::Duration::from_secs(5 * successive_failures as u64);
                    error!("{} Connection error {}\nRetrying in {} seconds...", successive_failures, error.to_string(), sleep_time.as_secs());
                    // Sleep for 5 seconds for each successive failure
                    thread::sleep(sleep_time);
                    client.subscribe("home/leds", QoS::AtLeastOnce).unwrap();
                }
            }
        }
    });

    let mut controller: rs_ws281x::Controller = match ControllerBuilder::new()
        // default
        .freq(800_000)
        // default
        .dma(10)
        .channel(
            0,
            ChannelBuilder::new()
                .pin(18)
                .count(STRIP_LENGTH)
                .strip_type(StripType::Ws2811Rgb)
                .brightness(127)
                .build()
        )
        .build() {
        Ok(c) => c,
        Err(e) => {
            panic!("Unable to setup led controller: {}", e);
        }
    };

    loop {
        // Check if current animation is different to the next animation and that the current animation is not stopping
        if current_animation.name().ne(next_animation_name.lock().unwrap().as_str()) && !current_animation.stopping() {
            info!("Stopping animation: {}", current_animation.name());
            current_animation.stop();
        }

        // Save the result of next_frame to a variable so that we can check if the animation has changed
        let res: bool = current_animation.next_frame(&mut controller);

        // If the animation stopped, we can use next_animation to start the next animation
        if !res {
            let next_animation = match next_animation_name.lock() {
                Ok(n) => n,
                Err(e) => {
                    error!("Unable to lock next_animation_name: {}", e);
                    continue;
                }
            };

            // If the next animation is not empty, we can start it
            if !next_animation.is_empty() {
                info!("Starting animation: {}", next_animation);

                // Get the animation factory from the hashmap
                let animation_factory = match animation_factories.get(next_animation.as_str()) {
                    Some(f) => f,
                    None => {
                        warn!("Unable to find animation factory for animation: `{}` defaulting to off", next_animation);
                        animation_factories.get("off").unwrap()
                    }
                };

                // Create the new animation
                current_animation = animation_factory();
                current_animation.start();
            }
        }

        controller.render().unwrap();
        thread::sleep(time::Duration::from_millis(current_animation.wait_time()));
    }
}
