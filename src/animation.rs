use rs_ws281x::Controller;

pub trait Animation {
    /// Computes and renders the next frame of the animation to the controller
    /// ## Arguments
    /// * `controller` - The controller to render the next frame to
    /// ## Returns
    /// * `bool` - True if the animation is still running, false otherwise
    fn next_frame(&mut self, controller: &mut Controller) -> bool;

    /// Starts the animation
    fn start(&mut self) -> ();

    /// Stops the animation (the animation is allowed to finish)
    fn stop(&mut self) -> ();

    /// Returns true if the animation is stopping, false otherwise
    fn stopping(&self) -> bool;

    /// Returns the name of the animation
    fn name(&self) -> &str;

    /// Time to wait between to animation frames
    fn wait_time(&self) -> u64;
}