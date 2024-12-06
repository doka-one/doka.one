use crate::{HarborContext, MapToHarbor};
use dkdto::{EnumTagValue, GetItemReply, ItemElement, TagValueElement};
use serde_derive::{Deserialize, Serialize};

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
