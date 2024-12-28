use crate::{
    ActiveTooltip, AnyTooltip, AnyView, AppContext, Bounds, DispatchPhase, Element, ElementId,
    GlobalElementId, HighlightStyle, Hitbox, IntoElement, LayoutId, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, SharedString, Size, TextRun, TextStyle, Truncate, WhiteSpace,
    Window, WrappedLine, TOOLTIP_DELAY,
};
use anyhow::anyhow;
use parking_lot::{Mutex, MutexGuard};
use smallvec::SmallVec;
use std::{
    cell::{Cell, RefCell},
    mem,
    ops::Range,
    rc::Rc,
    sync::Arc,
};
use util::ResultExt;

impl Element for &'static str {
    type RequestLayoutState = TextLayout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        window: &mut Window,
        cx: &mut AppContext,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut state = TextLayout::default();
        let layout_id = state.layout(SharedString::from(*self), None, window, cx);
        (layout_id, state)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        text_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut AppContext,
    ) {
        text_layout.prepaint(bounds, self)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _bounds: Bounds<Pixels>,
        text_layout: &mut TextLayout,
        _: &mut (),
        window: &mut Window,
        cx: &mut AppContext,
    ) {
        text_layout.paint(self, window, cx)
    }
}

impl IntoElement for &'static str {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl IntoElement for String {
    type Element = SharedString;

    fn into_element(self) -> Self::Element {
        self.into()
    }
}

impl Element for SharedString {
    type RequestLayoutState = TextLayout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn request_layout(
        &mut self,

        _id: Option<&GlobalElementId>,

        window: &mut Window,
        cx: &mut AppContext,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut state = TextLayout::default();
        let layout_id = state.layout(self.clone(), None, window, cx);
        (layout_id, state)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        text_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut AppContext,
    ) {
        text_layout.prepaint(bounds, self.as_ref())
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _bounds: Bounds<Pixels>,
        text_layout: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut AppContext,
    ) {
        text_layout.paint(self.as_ref(), window, cx)
    }
}

impl IntoElement for SharedString {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Renders text with runs of different styles.
///
/// Callers are responsible for setting the correct style for each run.
/// For text with a uniform style, you can usually avoid calling this constructor
/// and just pass text directly.
pub struct StyledText {
    text: SharedString,
    runs: Option<Vec<TextRun>>,
    layout: TextLayout,
}

impl StyledText {
    /// Construct a new styled text element from the given string.
    pub fn new(text: impl Into<SharedString>) -> Self {
        StyledText {
            text: text.into(),
            runs: None,
            layout: TextLayout::default(),
        }
    }

    /// Get the layout for this element. This can be used to map indices to pixels and vice versa.
    pub fn layout(&self) -> &TextLayout {
        &self.layout
    }

    /// Set the styling attributes for the given text, as well as
    /// as any ranges of text that have had their style customized.
    pub fn with_highlights(
        mut self,
        default_style: &TextStyle,
        highlights: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    ) -> Self {
        let mut runs = Vec::new();
        let mut ix = 0;
        for (range, highlight) in highlights {
            if ix < range.start {
                runs.push(default_style.clone().to_run(range.start - ix));
            }
            runs.push(
                default_style
                    .clone()
                    .highlight(highlight)
                    .to_run(range.len()),
            );
            ix = range.end;
        }
        if ix < self.text.len() {
            runs.push(default_style.to_run(self.text.len() - ix));
        }
        self.runs = Some(runs);
        self
    }

    /// Set the text runs for this piece of text.
    pub fn with_runs(mut self, runs: Vec<TextRun>) -> Self {
        self.runs = Some(runs);
        self
    }
}

impl Element for StyledText {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn request_layout(
        &mut self,

        _id: Option<&GlobalElementId>,

        window: &mut Window,
        cx: &mut AppContext,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self
            .layout
            .layout(self.text.clone(), self.runs.take(), window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut AppContext,
    ) {
        self.layout.prepaint(bounds, &self.text)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut AppContext,
    ) {
        self.layout.paint(&self.text, window, cx)
    }
}

impl IntoElement for StyledText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// The Layout for TextElement. This can be used to map indices to pixels and vice versa.
#[derive(Default, Clone)]
pub struct TextLayout(Arc<Mutex<Option<TextLayoutInner>>>);

struct TextLayoutInner {
    lines: SmallVec<[WrappedLine; 1]>,
    line_height: Pixels,
    wrap_width: Option<Pixels>,
    size: Option<Size<Pixels>>,
    bounds: Option<Bounds<Pixels>>,
}

const ELLIPSIS: &str = "…";

impl TextLayout {
    fn lock(&self) -> MutexGuard<Option<TextLayoutInner>> {
        self.0.lock()
    }

    fn layout(
        &self,
        text: SharedString,
        runs: Option<Vec<TextRun>>,
        window: &mut Window,
        cx: &mut AppContext,
    ) -> LayoutId {
        let text_style = window.text_style(cx);
        let font_size = text_style.font_size.to_pixels(window.rem_size(cx));
        let line_height = text_style
            .line_height
            .to_pixels(font_size.into(), window.rem_size(cx));

        let mut runs = if let Some(runs) = runs {
            runs
        } else {
            vec![text_style.to_run(text.len())]
        };

        let layout_id = window.request_measured_layout(
            Default::default(),
            {
                let element_state = self.clone();

                move |known_dimensions, available_space, window, cx| {
                    let wrap_width = if text_style.white_space == WhiteSpace::Normal {
                        known_dimensions.width.or(match available_space.width {
                            crate::AvailableSpace::Definite(x) => Some(x),
                            _ => None,
                        })
                    } else {
                        None
                    };

                    let (truncate_width, ellipsis) = if let Some(truncate) = text_style.truncate {
                        let width = known_dimensions.width.or(match available_space.width {
                            crate::AvailableSpace::Definite(x) => Some(x),
                            _ => None,
                        });

                        match truncate {
                            Truncate::Truncate => (width, None),
                            Truncate::Ellipsis => (width, Some(ELLIPSIS)),
                        }
                    } else {
                        (None, None)
                    };

                    if let Some(text_layout) = element_state.0.lock().as_ref() {
                        if text_layout.size.is_some()
                            && (wrap_width.is_none() || wrap_width == text_layout.wrap_width)
                        {
                            return text_layout.size.unwrap();
                        }
                    }

                    let mut line_wrapper = window
                        .text_system(cx)
                        .line_wrapper(text_style.font(), font_size);
                    let text = if let Some(truncate_width) = truncate_width {
                        line_wrapper.truncate_line(
                            text.clone(),
                            truncate_width,
                            ellipsis,
                            &mut runs,
                        )
                    } else {
                        text.clone()
                    };

                    let Some(lines) = window
                        .text_system(cx)
                        .shape_text(
                            text, font_size, &runs, wrap_width, // Wrap if we know the width.
                        )
                        .log_err()
                    else {
                        element_state.lock().replace(TextLayoutInner {
                            lines: Default::default(),
                            line_height,
                            wrap_width,
                            size: Some(Size::default()),
                            bounds: None,
                        });
                        return Size::default();
                    };

                    let mut size: Size<Pixels> = Size::default();
                    for line in &lines {
                        let line_size = line.size(line_height);
                        size.height += line_size.height;
                        size.width = size.width.max(line_size.width).ceil();
                    }

                    element_state.lock().replace(TextLayoutInner {
                        lines,
                        line_height,
                        wrap_width,
                        size: Some(size),
                        bounds: None,
                    });

                    size
                }
            },
            cx,
        );

        layout_id
    }

    fn prepaint(&self, bounds: Bounds<Pixels>, text: &str) {
        let mut element_state = self.lock();
        let element_state = element_state
            .as_mut()
            .ok_or_else(|| anyhow!("measurement has not been performed on {}", text))
            .unwrap();
        element_state.bounds = Some(bounds);
    }

    fn paint(&self, text: &str, window: &mut Window, cx: &mut AppContext) {
        let element_state = self.lock();
        let element_state = element_state
            .as_ref()
            .ok_or_else(|| anyhow!("measurement has not been performed on {}", text))
            .unwrap();
        let bounds = element_state
            .bounds
            .ok_or_else(|| anyhow!("prepaint has not been performed on {:?}", text))
            .unwrap();

        let line_height = element_state.line_height;
        let mut line_origin = bounds.origin;
        for line in &element_state.lines {
            line.paint(line_origin, line_height, window, cx).log_err();
            line_origin.y += line.size(line_height).height;
        }
    }

    /// Get the byte index into the input of the pixel position.
    pub fn index_for_position(&self, mut position: Point<Pixels>) -> Result<usize, usize> {
        let element_state = self.lock();
        let element_state = element_state
            .as_ref()
            .expect("measurement has not been performed");
        let bounds = element_state
            .bounds
            .expect("prepaint has not been performed");

        if position.y < bounds.top() {
            return Err(0);
        }

        let line_height = element_state.line_height;
        let mut line_origin = bounds.origin;
        let mut line_start_ix = 0;
        for line in &element_state.lines {
            let line_bottom = line_origin.y + line.size(line_height).height;
            if position.y > line_bottom {
                line_origin.y = line_bottom;
                line_start_ix += line.len() + 1;
            } else {
                let position_within_line = position - line_origin;
                match line.index_for_position(position_within_line, line_height) {
                    Ok(index_within_line) => return Ok(line_start_ix + index_within_line),
                    Err(index_within_line) => return Err(line_start_ix + index_within_line),
                }
            }
        }

        Err(line_start_ix.saturating_sub(1))
    }

    /// Get the pixel position for the given byte index.
    pub fn position_for_index(&self, index: usize) -> Option<Point<Pixels>> {
        let element_state = self.lock();
        let element_state = element_state
            .as_ref()
            .expect("measurement has not been performed");
        let bounds = element_state
            .bounds
            .expect("prepaint has not been performed");
        let line_height = element_state.line_height;

        let mut line_origin = bounds.origin;
        let mut line_start_ix = 0;

        for line in &element_state.lines {
            let line_end_ix = line_start_ix + line.len();
            if index < line_start_ix {
                break;
            } else if index > line_end_ix {
                line_origin.y += line.size(line_height).height;
                line_start_ix = line_end_ix + 1;
                continue;
            } else {
                let ix_within_line = index - line_start_ix;
                return Some(line_origin + line.position_for_index(ix_within_line, line_height)?);
            }
        }

        None
    }

    /// The bounds of this layout.
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.0.lock().as_ref().unwrap().bounds.unwrap()
    }

    /// The line height for this layout.
    pub fn line_height(&self) -> Pixels {
        self.0.lock().as_ref().unwrap().line_height
    }

    /// The text for this layout.
    pub fn text(&self) -> String {
        self.0
            .lock()
            .as_ref()
            .unwrap()
            .lines
            .iter()
            .map(|s| s.text.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A text element that can be interacted with.
pub struct InteractiveText {
    element_id: ElementId,
    text: StyledText,
    click_listener: Option<
        Box<dyn Fn(&[Range<usize>], InteractiveTextClickEvent, &mut Window, &mut AppContext)>,
    >,
    hover_listener:
        Option<Box<dyn Fn(Option<usize>, MouseMoveEvent, &mut Window, &mut AppContext)>>,
    tooltip_builder: Option<Rc<dyn Fn(usize, &mut Window, &mut AppContext) -> Option<AnyView>>>,
    clickable_ranges: Vec<Range<usize>>,
}

struct InteractiveTextClickEvent {
    mouse_down_index: usize,
    mouse_up_index: usize,
}

#[doc(hidden)]
#[derive(Default)]
pub struct InteractiveTextState {
    mouse_down_index: Rc<Cell<Option<usize>>>,
    hovered_index: Rc<Cell<Option<usize>>>,
    active_tooltip: Rc<RefCell<Option<ActiveTooltip>>>,
}

/// InteractiveTest is a wrapper around StyledText that adds mouse interactions.
impl InteractiveText {
    /// Creates a new InteractiveText from the given text.
    pub fn new(id: impl Into<ElementId>, text: StyledText) -> Self {
        Self {
            element_id: id.into(),
            text,
            click_listener: None,
            hover_listener: None,
            tooltip_builder: None,
            clickable_ranges: Vec::new(),
        }
    }

    /// on_click is called when the user clicks on one of the given ranges, passing the index of
    /// the clicked range.
    pub fn on_click(
        mut self,
        ranges: Vec<Range<usize>>,
        listener: impl Fn(usize, &mut Window, &mut AppContext) + 'static,
    ) -> Self {
        self.click_listener = Some(Box::new(move |ranges, event, window, cx| {
            for (range_ix, range) in ranges.iter().enumerate() {
                if range.contains(&event.mouse_down_index) && range.contains(&event.mouse_up_index)
                {
                    listener(range_ix, window, cx);
                }
            }
        }));
        self.clickable_ranges = ranges;
        self
    }

    /// on_hover is called when the mouse moves over a character within the text, passing the
    /// index of the hovered character, or None if the mouse leaves the text.
    pub fn on_hover(
        mut self,
        listener: impl Fn(Option<usize>, MouseMoveEvent, &mut Window, &mut AppContext) + 'static,
    ) -> Self {
        self.hover_listener = Some(Box::new(listener));
        self
    }

    /// tooltip lets you specify a tooltip for a given character index in the string.
    pub fn tooltip(
        mut self,
        builder: impl Fn(usize, &mut Window, &mut AppContext) -> Option<AnyView> + 'static,
    ) -> Self {
        self.tooltip_builder = Some(Rc::new(builder));
        self
    }
}

impl Element for InteractiveText {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.element_id.clone())
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        window: &mut Window,
        cx: &mut AppContext,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.text.request_layout(None, window, cx)
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut AppContext,
    ) -> Hitbox {
        window.with_optional_element_state::<InteractiveTextState, _>(
            global_id,
            cx,
            |interactive_state, window, cx| {
                let interactive_state = interactive_state
                    .map(|interactive_state| interactive_state.unwrap_or_default());

                if let Some(interactive_state) = interactive_state.as_ref() {
                    if let Some(active_tooltip) = interactive_state.active_tooltip.borrow().as_ref()
                    {
                        if let Some(tooltip) = active_tooltip.tooltip.clone() {
                            window.set_tooltip(tooltip, cx);
                        }
                    }
                }

                self.text.prepaint(None, bounds, state, window, cx);
                let hitbox = window.insert_hitbox(bounds, false, cx);
                (hitbox, interactive_state)
            },
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut AppContext,
    ) {
        let text_layout = self.text.layout().clone();
        window.with_element_state::<InteractiveTextState, _>(
            global_id.unwrap(),
            cx,
            |interactive_state, window, cx| {
                let mut interactive_state = interactive_state.unwrap_or_default();
                if let Some(click_listener) = self.click_listener.take() {
                    let mouse_position = window.mouse_position(cx);
                    if let Ok(ix) = text_layout.index_for_position(mouse_position) {
                        if self
                            .clickable_ranges
                            .iter()
                            .any(|range| range.contains(&ix))
                        {
                            window.set_cursor_style(crate::CursorStyle::PointingHand, hitbox, cx)
                        }
                    }

                    let text_layout = text_layout.clone();
                    let mouse_down = interactive_state.mouse_down_index.clone();
                    if let Some(mouse_down_index) = mouse_down.get() {
                        let hitbox = hitbox.clone();
                        let clickable_ranges = mem::take(&mut self.clickable_ranges);
                        window.on_mouse_event(
                            cx,
                            move |event: &MouseUpEvent, phase, window, cx| {
                                if phase == DispatchPhase::Bubble && hitbox.is_hovered(window, cx) {
                                    if let Ok(mouse_up_index) =
                                        text_layout.index_for_position(event.position)
                                    {
                                        click_listener(
                                            &clickable_ranges,
                                            InteractiveTextClickEvent {
                                                mouse_down_index,
                                                mouse_up_index,
                                            },
                                            window,
                                            cx,
                                        )
                                    }

                                    mouse_down.take();
                                    window.refresh(cx);
                                }
                            },
                        );
                    } else {
                        let hitbox = hitbox.clone();
                        window.on_mouse_event(
                            cx,
                            move |event: &MouseDownEvent, phase, window, cx| {
                                if phase == DispatchPhase::Bubble && hitbox.is_hovered(window, cx) {
                                    if let Ok(mouse_down_index) =
                                        text_layout.index_for_position(event.position)
                                    {
                                        mouse_down.set(Some(mouse_down_index));
                                        window.refresh(cx);
                                    }
                                }
                            },
                        );
                    }
                }

                window.on_mouse_event(cx, {
                    let mut hover_listener = self.hover_listener.take();
                    let hitbox = hitbox.clone();
                    let text_layout = text_layout.clone();
                    let hovered_index = interactive_state.hovered_index.clone();
                    move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble && hitbox.is_hovered(window, cx) {
                            let current = hovered_index.get();
                            let updated = text_layout.index_for_position(event.position).ok();
                            if current != updated {
                                hovered_index.set(updated);
                                if let Some(hover_listener) = hover_listener.as_ref() {
                                    hover_listener(updated, event.clone(), window, cx);
                                }
                                window.refresh(cx);
                            }
                        }
                    }
                });

                if let Some(tooltip_builder) = self.tooltip_builder.clone() {
                    let hitbox = hitbox.clone();
                    let active_tooltip = interactive_state.active_tooltip.clone();
                    let pending_mouse_down = interactive_state.mouse_down_index.clone();
                    let text_layout = text_layout.clone();

                    window.on_mouse_event(cx, move |event: &MouseMoveEvent, phase, window, cx| {
                        let position = text_layout.index_for_position(event.position).ok();
                        let is_hovered = position.is_some()
                            && hitbox.is_hovered(window, cx)
                            && pending_mouse_down.get().is_none();
                        if !is_hovered {
                            active_tooltip.take();
                            return;
                        }
                        let position = position.unwrap();

                        if phase != DispatchPhase::Bubble {
                            return;
                        }

                        if active_tooltip.borrow().is_none() {
                            let task = window.spawn(cx, {
                                let active_tooltip = active_tooltip.clone();
                                let tooltip_builder = tooltip_builder.clone();

                                move |mut cx| async move {
                                    cx.background_executor().timer(TOOLTIP_DELAY).await;
                                    cx.update(|window, cx| {
                                        let new_tooltip = tooltip_builder(position, window, cx)
                                            .map(|tooltip| ActiveTooltip {
                                                tooltip: Some(AnyTooltip {
                                                    view: tooltip,
                                                    mouse_position: window.mouse_position(cx),
                                                }),
                                                _task: None,
                                            });
                                        *active_tooltip.borrow_mut() = new_tooltip;
                                        window.refresh(cx);
                                    })
                                    .ok();
                                }
                            });
                            *active_tooltip.borrow_mut() = Some(ActiveTooltip {
                                tooltip: None,
                                _task: Some(task),
                            });
                        }
                    });

                    let active_tooltip = interactive_state.active_tooltip.clone();
                    window.on_mouse_event(cx, move |_: &MouseDownEvent, _, _| {
                        active_tooltip.take();
                    });
                }

                self.text.paint(None, bounds, &mut (), &mut (), window, cx);

                ((), interactive_state)
            },
        );
    }
}

impl IntoElement for InteractiveText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
