use druid::{
    BoxConstraints, Cursor, Env, Event, EventCtx, LayoutCtx, LifeCycle, LifeCycleCtx,
    MouseEvent, PaintCtx, Point, Size, UpdateCtx, Widget, WidgetExt,
};
use lapce_data::{data::LapceTabData, outline::OutlineData, panel::PanelKind};

use crate::panel::{LapcePanel, PanelHeaderKind, PanelSizing};

pub fn new_outline_panel(data: &OutlineData) -> LapcePanel {
    LapcePanel::new(
        PanelKind::Outline,
        data.widget_id,
        data.split_id,
        vec![
            (
                data.file_outline_widget_id,
                PanelHeaderKind::None,
                OutlineContent::new().boxed(),
                PanelSizing::Flex(false),
            )
        ],
    )
}

struct OutlineContent {
    mouse_pos: Point,
    content_height: f64,
}

impl OutlineContent {
    pub fn new() -> Self {
        Self {
            mouse_pos: Point::ZERO,
            content_height: 0.0,
        }
    }

    fn mouse_down(
        &self,
        ctx: &mut EventCtx,
        _mouse_event: &MouseEvent,
        _data: &LapceTabData,
    ) {
        // If it isn't hot then we don't bother checking
        if !ctx.is_hot() {
            return;
        }
    }
}

impl Widget<LapceTabData> for OutlineContent {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut LapceTabData,
        _env: &Env,
    ) {
        match event {
            Event::MouseMove(mouse_event) => {
                self.mouse_pos = mouse_event.pos;
                
                if mouse_event.pos.y < self.content_height {
                    ctx.set_cursor(&Cursor::Pointer);
                } else {
                    ctx.clear_cursor();
                }
                
                ctx.request_paint();
            }
            Event::MouseDown(mouse_event) => {
                self.mouse_down(ctx, mouse_event, data);
            }
            _ => {}
        }
    }

    fn lifecycle(
        &mut self,
        _ctx: &mut LifeCycleCtx,
        _event: &LifeCycle,
        _data: &LapceTabData,
        _env: &Env,
    ) {
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        old_data: &LapceTabData,
        data: &LapceTabData,
        _env: &Env,
    ) {
        if data.main_split.active_tab != old_data.main_split.active_tab {
            ctx.request_layout();
        }
    }

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &LapceTabData,
        _env: &Env,
    ) -> Size {
        Size::new(bc.max().width, self.content_height.max(bc.max().height))
    }

    fn paint(&mut self, _ctx: &mut PaintCtx, _data: &LapceTabData, _env: &Env) {}
}
