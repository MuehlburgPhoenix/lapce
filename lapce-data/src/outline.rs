use druid::WidgetId;

#[derive(Clone)]
pub struct OutlineData {
    pub widget_id: WidgetId,
    pub split_id: WidgetId,
    pub file_outline_widget_id: WidgetId,
}

impl OutlineData {
    pub fn new() -> Self {
        Self {
            widget_id: WidgetId::next(),
            split_id: WidgetId::next(),
            file_outline_widget_id: WidgetId::next(),
        }
    }
}

impl Default for OutlineData {
    fn default() -> Self {
        Self::new()
    }
}
