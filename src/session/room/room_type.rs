use std::convert::TryFrom;

use gtk::glib;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::session::sidebar::CategoryType;

// TODO: do we also want the category `People` and a custom category support?
#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, glib::Enum, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
#[enum_type(name = "RoomType")]
pub enum RoomType {
    Invited = 0,
    Favorite = 1,
    Normal = 2,
    LowPriority = 3,
    Left = 4,
    Outdated = 5,
}

impl RoomType {
    /// Check whether this `RoomType` can be changed to `category`.
    pub fn can_change_to(&self, category: &RoomType) -> bool {
        match self {
            Self::Invited => {
                matches!(
                    category,
                    Self::Favorite | Self::Normal | Self::LowPriority | Self::Left
                )
            }
            Self::Favorite => {
                matches!(category, Self::Normal | Self::LowPriority | Self::Left)
            }
            Self::Normal => {
                matches!(category, Self::Favorite | Self::LowPriority | Self::Left)
            }
            Self::LowPriority => {
                matches!(category, Self::Favorite | Self::Normal | Self::Left)
            }
            Self::Left => {
                matches!(category, Self::Favorite | Self::Normal | Self::LowPriority)
            }
            Self::Outdated => false,
        }
    }
}

impl Default for RoomType {
    fn default() -> Self {
        RoomType::Normal
    }
}

impl ToString for RoomType {
    fn to_string(&self) -> String {
        CategoryType::from(self).to_string()
    }
}

impl TryFrom<CategoryType> for RoomType {
    type Error = &'static str;

    fn try_from(category_type: CategoryType) -> Result<Self, Self::Error> {
        Self::try_from(&category_type)
    }
}

impl TryFrom<&CategoryType> for RoomType {
    type Error = &'static str;

    fn try_from(category_type: &CategoryType) -> Result<Self, Self::Error> {
        match category_type {
            CategoryType::None => Err("CategoryType::None cannot be a RoomType"),
            CategoryType::Invited => Ok(Self::Invited),
            CategoryType::Favorite => Ok(Self::Favorite),
            CategoryType::Normal => Ok(Self::Normal),
            CategoryType::LowPriority => Ok(Self::LowPriority),
            CategoryType::Left => Ok(Self::Left),
            CategoryType::Outdated => Ok(Self::Outdated),
            CategoryType::VerificationRequest => {
                Err("CategoryType::VerificationRequest cannot be a RoomType")
            }
        }
    }
}
