use crate::{HarborContext, MapToHarbor};
use dkdto::{EnumTagValue, GetItemReply, ItemElement, TagValueElement};
use serde_derive::{Deserialize, Serialize};

/// Model for the SearchResult component

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchResultHarbor {
    pub items: Vec<ItemHarbor>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ItemHarbor {
    pub item_id: i64,
    pub name: String,
    pub file_ref: Option<String>,
    pub properties: Vec<KeyValue>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct KeyValue {
    pub key: String,
    pub value: Option<String>,
}

impl MapToHarbor<SearchResultHarbor> for GetItemReply {
    fn map_to_harbor(&self, context: &HarborContext) -> SearchResultHarbor {
        SearchResultHarbor {
            items: self
                .items
                .iter()
                .map(|x| x.map_to_harbor(&context))
                .collect(),
        }
    }
}

impl MapToHarbor<ItemHarbor> for ItemElement {
    fn map_to_harbor(&self, context: &HarborContext) -> ItemHarbor {
        let datetime_format_fn = context.datetime_format_fn;

        let mut key_values: Vec<KeyValue> = match self.properties.as_ref() {
            None => {
                vec![]
            }
            Some(props) => props.iter().map(|x| x.map_to_harbor(&context)).collect(),
        };

        key_values.push(KeyValue {
            key: "Nom".to_string(),
            value: Some(self.name.clone()),
        });

        key_values.push(KeyValue {
            key: "Date de création".to_string(),
            value: Some(datetime_format_fn(&self.created, 1)),
        });

        key_values.push(KeyValue {
            key: "Date de modification".to_string(),
            value: self
                .last_modified
                .as_ref()
                .map(|v| datetime_format_fn(v, 1)),
        });

        ItemHarbor {
            item_id: self.item_id,
            name: self.name.clone(),
            file_ref: self.file_ref.clone(),
            properties: key_values,
        }
    }
}

impl MapToHarbor<KeyValue> for TagValueElement {
    fn map_to_harbor(&self, context: &HarborContext) -> KeyValue {
        KeyValue {
            key: self.tag_name.clone(),
            value: self.value.map_to_harbor(&context),
        }
    }
}

impl MapToHarbor<Option<String>> for EnumTagValue {
    fn map_to_harbor(&self, context: &HarborContext) -> Option<String> {
        let date_format_fn = context.date_format_fn;

        match self {
            EnumTagValue::Text(v) => v.clone(), // Already an Option<String>, so just clone it
            EnumTagValue::Boolean(v) => v.as_ref().map(|vv| vv.to_string()), // Convert bool to string
            EnumTagValue::Integer(v) => v.as_ref().map(|vv| vv.to_string()), // Convert integer to string
            EnumTagValue::Double(v) => v.as_ref().map(|vv| vv.to_string()), // Convert double to string
            EnumTagValue::SimpleDate(v) => v.as_ref().map(|vv| date_format_fn(&vv)), // Format date using context
            EnumTagValue::DateTime(v) => v.as_ref().map(|vv| date_format_fn(&vv)), // Format datetime using context
            EnumTagValue::Link(v) => v.as_ref().map(|vv| vv.to_string()), // Convert link to string
        }
    }
}

/// End Model for the SearchResult component

#[derive(Serialize, Deserialize, Debug)]
pub struct GetItemReplyForComponent1 {
    pub items: Vec<ItemElementForComponent1>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ItemElementForComponent1 {
    pub item_id: i64,
    pub name: String,
    pub file_ref: Option<String>,
    pub created_iso: String,
    pub created_formatted: String,
    pub last_modified_iso: Option<String>,
    pub last_modified_formatted: Option<String>,
    pub properties: Option<Vec<TagValueElementForComponent1>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TagValueElementForComponent1 {
    pub tag_name: String,
    pub value: EnumTagValue,
    pub formatted_value: String,
}

impl MapToHarbor<GetItemReplyForComponent1> for GetItemReply {
    fn map_to_harbor(&self, context: &HarborContext) -> GetItemReplyForComponent1 {
        GetItemReplyForComponent1 {
            items: self
                .items
                .iter()
                .map(|x| x.map_to_harbor(&context))
                .collect(),
        }
    }
}

impl MapToHarbor<ItemElementForComponent1> for ItemElement {
    fn map_to_harbor(&self, context: &HarborContext) -> ItemElementForComponent1 {
        let datetime_format_fn = context.datetime_format_fn;
        ItemElementForComponent1 {
            item_id: self.item_id,
            name: self.name.clone(),
            file_ref: self.file_ref.clone(),
            created_iso: self.created.clone(),
            created_formatted: datetime_format_fn(&self.created, 1),
            last_modified_iso: self.last_modified.clone(),
            // Map the last_modified field if it's Some, else return None
            last_modified_formatted: self
                .last_modified
                .as_ref()
                .map(|x| datetime_format_fn(x, 1)),
            properties: self
                .properties
                .as_ref()
                .map(|x| x.iter().map(|y| y.map_to_harbor(&context)).collect()),
        }
    }
}

impl MapToHarbor<TagValueElementForComponent1> for TagValueElement {
    fn map_to_harbor(&self, context: &HarborContext) -> TagValueElementForComponent1 {
        let date_format_fn = context.date_format_fn;
        TagValueElementForComponent1 {
            tag_name: self.tag_name.clone(),
            value: self.value.clone(),
            formatted_value: date_format_fn("2024-04-29"),
        }
    }
}
