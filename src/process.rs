pub enum ProcessState {
    Running,
    /// The process is sleeping for a certain amount of time.
    Sleeping,
    /// The process is wating on I/O.
    Waiting,
}
