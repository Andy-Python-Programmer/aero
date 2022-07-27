#[repr(C)]
pub struct TimeVal {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

#[repr(C)]
pub struct ITimerVal {
    pub it_interval: TimeVal, // Interval for periodic timer
    pub it_value: TimeVal,    // Time until next expiration
}
