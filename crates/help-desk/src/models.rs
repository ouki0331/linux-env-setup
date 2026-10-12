use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Japanese,
    English,
}

impl Language {
    pub fn toggle(&self) -> Self {
        match self {
            Language::Japanese => Language::English,
            Language::English => Language::Japanese,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Language::Japanese => "JA (日本語)",
            Language::English => "EN (English)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Alphabetical,
    Category,
    Date,
}

impl SortMode {
    pub fn cycle(&self) -> Self {
        match self {
            SortMode::Alphabetical => SortMode::Category,
            SortMode::Category => SortMode::Date,
            SortMode::Date => SortMode::Alphabetical,
        }
    }

    pub fn label(&self, lang: Language) -> &'static str {
        match (self, lang) {
            (SortMode::Alphabetical, Language::Japanese) => "名前順 (A-Z)",
            (SortMode::Alphabetical, Language::English) => "Alphabetical",
            (SortMode::Category, Language::Japanese) => "分類順 (カテゴリ)",
            (SortMode::Category, Language::English) => "Category",
            (SortMode::Date, Language::Japanese) => "日付順 (更新日時)",
            (SortMode::Date, Language::English) => "Date",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn toggle(&self) -> Self {
        match self {
            SortOrder::Ascending => SortOrder::Descending,
            SortOrder::Descending => SortOrder::Ascending,
        }
    }

    pub fn arrow(&self) -> &'static str {
        match self {
            SortOrder::Ascending => "▲ ASC",
            SortOrder::Descending => "▼ DESC",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    LeftPanel,
    RightPanel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionDoc {
    pub flag: String,
    #[serde(default)]
    pub desc_ja: String,
    #[serde(default)]
    pub desc_en: String,
}

impl OptionDoc {
    pub fn desc(&self, lang: Language) -> &str {
        match lang {
            Language::Japanese => {
                if !self.desc_ja.is_empty() {
                    &self.desc_ja
                } else {
                    &self.desc_en
                }
            }
            Language::English => {
                if !self.desc_en.is_empty() {
                    &self.desc_en
                } else {
                    &self.desc_ja
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleDoc {
    #[serde(default)]
    pub description_ja: String,
    #[serde(default)]
    pub description_en: String,
    pub command: String,
}

impl ExampleDoc {
    pub fn desc(&self, lang: Language) -> &str {
        match lang {
            Language::Japanese => {
                if !self.description_ja.is_empty() {
                    &self.description_ja
                } else {
                    &self.description_en
                }
            }
            Language::English => {
                if !self.description_en.is_empty() {
                    &self.description_en
                } else {
                    &self.description_ja
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDoc {
    pub command: String,
    #[serde(default)]
    pub category_ja: String,
    #[serde(default)]
    pub category_en: String,
    #[serde(default)]
    pub summary_ja: String,
    #[serde(default)]
    pub summary_en: String,
    #[serde(default)]
    pub description_ja: String,
    #[serde(default)]
    pub description_en: String,
    #[serde(default)]
    pub syntax: String,
    #[serde(default)]
    pub options: Vec<OptionDoc>,
    #[serde(default)]
    pub examples: Vec<ExampleDoc>,
    #[serde(default)]
    pub updated_at: String,
}

impl CommandDoc {
    pub fn category(&self, lang: Language) -> &str {
        match lang {
            Language::Japanese => {
                if !self.category_ja.is_empty() {
                    &self.category_ja
                } else {
                    &self.category_en
                }
            }
            Language::English => {
                if !self.category_en.is_empty() {
                    &self.category_en
                } else {
                    &self.category_ja
                }
            }
        }
    }

    pub fn summary(&self, lang: Language) -> &str {
        match lang {
            Language::Japanese => {
                if !self.summary_ja.is_empty() {
                    &self.summary_ja
                } else {
                    &self.summary_en
                }
            }
            Language::English => {
                if !self.summary_en.is_empty() {
                    &self.summary_en
                } else {
                    &self.summary_ja
                }
            }
        }
    }

    pub fn description(&self, lang: Language) -> &str {
        match lang {
            Language::Japanese => {
                if !self.description_ja.is_empty() {
                    &self.description_ja
                } else {
                    &self.description_en
                }
            }
            Language::English => {
                if !self.description_en.is_empty() {
                    &self.description_en
                } else {
                    &self.description_ja
                }
            }
        }
    }
}
