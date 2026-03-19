/// Information returned alongside an observation after a step.
#[derive(Clone, Debug, Default)]
pub struct StepInfo {
    /// Cumulative reward earned this episode.
    pub reward: f64,
    /// Whether the episode has terminated (game over).
    pub terminated: bool,
    /// Whether the episode was truncated (e.g. time limit).
    pub truncated: bool,
    /// Arbitrary key-value metadata.
    pub info: Vec<(String, String)>,
}

/// Gymnasium-compatible environment trait.
///
/// Mirrors the OpenAI Gymnasium API:
///   - `reset` returns an initial observation
///   - `step` takes an action and returns (observation, reward, terminated, truncated, info)
///   - `render` produces a frame for human viewing
///   - `close` tears down resources
///
/// `Obs` and `Act` are generic so downstream users can define their own
/// observation and action representations.
pub trait Env {
    /// Observation type (e.g. a frame buffer, RAM snapshot, etc.)
    type Obs;
    /// Action type (e.g. an enum of joystick inputs)
    type Act;

    /// Reset the environment to its initial state.
    /// Returns the initial observation.
    fn reset(&mut self, seed: Option<u64>) -> Self::Obs;

    /// Advance the environment by one action.
    /// Returns the new observation and associated step metadata.
    fn step(&mut self, action: Self::Act) -> (Self::Obs, StepInfo);

    /// Render the current state for human consumption.
    /// Returns raw RGBA pixel data (height * width * 4 bytes).
    fn render(&self) -> Vec<u8>;

    /// Return the number of available actions.
    fn action_space(&self) -> usize;

    /// Return the shape of the observation as (height, width, channels).
    fn observation_shape(&self) -> (usize, usize, usize);

    /// Tear down the environment and release resources.
    fn close(&mut self);
}
