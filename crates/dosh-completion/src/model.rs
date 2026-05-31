#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub value: String,
    pub description: Option<String>,
    pub kind: Option<String>,
    pub icon: Option<String>,
    pub insert_text: Option<String>,
    pub priority: Option<i64>,
}

impl CompletionItem {
    pub fn new(value: String, description: Option<String>) -> Self {
        Self {
            value,
            description,
            kind: None,
            icon: None,
            insert_text: None,
            priority: None,
        }
    }
}
