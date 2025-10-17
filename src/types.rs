use std::collections::HashMap;

pub type ParamRange = (usize, usize);
pub type CapturedParam = (String, ParamRange);
pub type RouteParams = HashMap<String, String>;
pub type RouteMatch = (u16, RouteParams);
