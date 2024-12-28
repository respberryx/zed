#![allow(missing_docs)]

use crate::prelude::*;

#[derive(IntoElement)]
pub struct ListSeparator;

impl RenderOnce for ListSeparator {
    fn render(self, window: &mut Window, cx: &mut AppContext) -> impl IntoElement {
        div()
            .h_px()
            .w_full()
            .my(DynamicSpacing::Base06.rems(window, cx))
            .bg(cx.theme().colors().border_variant)
    }
}
