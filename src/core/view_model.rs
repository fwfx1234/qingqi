use serde::{Deserialize, Serialize};

use crate::core::{command::Action, icon::IconRef, plugin::ListItem};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ViewModel {
    List {
        items: Vec<ListItem>,
        actions: Vec<Action>,
    },
    Detail {
        markdown: String,
        actions: Vec<Action>,
    },
    Form {
        fields: Vec<Field>,
        actions: Vec<Action>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Field {
    pub id: String,
    pub label: String,
    pub value: String,
    pub kind: FieldKind,
    pub icon: Option<IconRef>,
    pub required: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldKind {
    Text,
    Password,
    Multiline,
    Toggle,
}
