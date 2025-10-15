#[derive(Debug, Clone)]
pub enum SegmentPart {
    Literal(String),
    Param { name: String },
}

#[derive(Debug, Clone)]
pub struct SegmentPattern {
    pub parts: Vec<SegmentPart>,
}

impl PartialEq for SegmentPattern {
    fn eq(&self, other: &Self) -> bool {
        if self.parts.len() != other.parts.len() {
            return false;
        }
        for (a, b) in self.parts.iter().zip(other.parts.iter()) {
            match (a, b) {
                (SegmentPart::Literal(la), SegmentPart::Literal(lb)) => {
                    if la != lb {
                        return false;
                    }
                }
                (SegmentPart::Param { name: na, .. }, SegmentPart::Param { name: nb, .. }) => {
                    if na != nb {
                        return false;
                    }
                }
                _ => {
                    return false;
                }
            }
        }
        true
    }
}
