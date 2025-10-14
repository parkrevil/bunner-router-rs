pub type ErrorCode = u16;
pub type StaticString = &'static str;
pub type WorkerId = u32;

pub type RouteMatch = (u16, Vec<(String, (usize, usize))>);
