use std::{
    borrow::Cow, cell::RefCell, ffi::CStr, ops::RangeInclusive, rc::Rc, str::FromStr, sync::Arc,
};

use deno_core::{
    GarbageCollected, JsBuffer, OpState, Resource, ResourceId, ascii_str,
    cppgc::Ptr,
    error::AnyError,
    op2, thiserror,
    v8::{self, GetPropertyNamesArgs, cppgc::Member},
};
use crate::IcedJsxRoot;

#[derive(Clone)]
struct ViewFn(pub Arc<dyn Fn() -> iced::Element<'static, (), smudgy_theme::Theme, iced::Renderer>>);
struct ViewFnVec(pub RefCell<Vec<ViewFn>>);
type SmudgyIcedJsxRoot = IcedJsxRoot<'static, (), smudgy_theme::Theme, iced::Renderer>;

type ProgressBar = iced::widget::ProgressBar<'static, smudgy_theme::Theme>;

type Message = ();

impl GarbageCollected for ViewFn {
    fn get_name(&self) -> &'static CStr {
        c"IcedJsxComponent"
    }
}

impl GarbageCollected for ViewFnVec {
    fn get_name(&self) -> &'static CStr {
        c"IcedJsxComponentList"
    }
}

deno_core::extension!(
  iced_jsx,
  ops = [
    op_iced_jsx_create_widget,
    op_iced_jsx_remove_widget,
    op_iced_jsx_create_view_fn_vec,
    op_iced_jsx_push_to_view_fn_vec,
    op_iced_jsx_create_column,
    op_iced_jsx_create_row,
    op_iced_jsx_create_text,
    op_iced_jsx_create_progress_bar,
  ],
  esm_entry_point = "ext:iced_jsx/iced_jsx.ts",
  esm = [ dir "src/extension/ts", "iced_jsx.ts" ],
  options = {
    iced_jsx_root: SmudgyIcedJsxRoot
  },
  state = |state, options| {
    state.put::<SmudgyIcedJsxRoot>(options.iced_jsx_root);
  },
);

macro_rules! get_number_prop {
    ($scope:ident, $obj:ident, $name:expr) => {
        {
            let prop = ascii_str!($name).v8_string($scope).expect("Could not allocate string").into();
            $obj.get($scope, prop).and_then(|v| v.to_number($scope)).and_then(|v| v.number_value($scope))
        }
    }
}

macro_rules! get_string_prop {
    ($scope:ident, $obj:ident, $name:expr) => {
        {
            let prop = ascii_str!($name).v8_string($scope).expect("Could not allocate string").into();
            $obj.get($scope, prop).map(|v| v.to_rust_string_lossy($scope))
        }
    }
}

macro_rules! get_bool_prop {
    ($scope:ident, $obj:ident, $name:expr) => {
        {
            let prop = ascii_str!($name).v8_string($scope).expect("Could not allocate string").into();
            $obj.get($scope, prop).map(|v| v.boolean_value($scope))
        }
    }
}


macro_rules! iced_color_from_maybe_v8_string {
    ($str:expr) => {
        $str.and_then(|b| color_art::Color::from_str(&b).map(|c| iced::Color::from_rgba8(c.red(), c.green(), c.blue(), c.alpha() as f32)).ok())
    }
}


#[op2(fast)]
fn op_iced_jsx_create_widget(state: &mut OpState, #[string] name: &str, #[cppgc] widget: &ViewFn) {
    let iced_jsx_root = state.borrow::<SmudgyIcedJsxRoot>();
    IcedJsxRoot::insert(iced_jsx_root, name, widget.0.clone());
}

#[op2(fast)]
fn op_iced_jsx_remove_widget(state: &mut OpState, #[string] name: &str) {
    let iced_jsx_root = state.borrow::<SmudgyIcedJsxRoot>();
    iced_jsx_root.remove(name);
}

#[op2]
#[cppgc]
fn op_iced_jsx_create_view_fn_vec() -> ViewFnVec {
    ViewFnVec(RefCell::new(Vec::new()))
}

#[op2(fast)]
fn op_iced_jsx_push_to_view_fn_vec(#[cppgc] vec: &ViewFnVec, #[cppgc] child: &ViewFn) {
    vec.0.borrow_mut().push(child.clone());
}

#[op2]
#[cppgc]
fn op_iced_jsx_create_column(
    #[cppgc] children: &ViewFnVec,
    width: Option<f32>,
    height: Option<f32>,
    spacing: Option<f32>,
    padding: Option<f32>,
) -> ViewFn {
    let children = children.0.take();

    let mut attr_fns: Vec<
        Box<
            dyn Fn(
                iced::widget::Column<'static, Message, smudgy_theme::Theme, iced::Renderer>,
            )
                -> iced::widget::Column<'static, Message, smudgy_theme::Theme, iced::Renderer>,
        >,
    > = Vec::new();

    if let Some(width) = width {
        attr_fns.push(Box::new(
            move |column: iced::widget::Column<
                'static,
                Message,
                smudgy_theme::Theme,
                iced::Renderer,
            >| column.width(width),
        ));
    }
    if let Some(height) = height {
        attr_fns.push(Box::new(
            move |column: iced::widget::Column<
                'static,
                Message,
                smudgy_theme::Theme,
                iced::Renderer,
            >| column.height(height),
        ));
    }

    if let Some(spacing) = spacing {
        attr_fns.push(Box::new(
            move |column: iced::widget::Column<
                'static,
                Message,
                smudgy_theme::Theme,
                iced::Renderer,
            >| column.spacing(spacing),
        ));
    }
    if let Some(padding) = padding {
        attr_fns.push(Box::new(
            move |column: iced::widget::Column<
                'static,
                Message,
                smudgy_theme::Theme,
                iced::Renderer,
            >| column.padding(padding),
        ));
    }

    ViewFn(Arc::new(move || {
        let column = iced::widget::column(children.iter().map(|c| c.0()));
        let column = attr_fns
            .iter()
            .fold(column, |column, attr_fn| attr_fn(column));
        column.into()
    }))
}

#[op2]
#[cppgc]
fn op_iced_jsx_create_progress_bar(
    scope: &mut v8::HandleScope,
    props: v8::Local<v8::Object>,
) -> ViewFn {
    let mut attr_fns: Vec<
    Box<
        dyn Fn(
            ProgressBar,
        )
            -> ProgressBar,
    >,
> = Vec::new();

    let min = get_number_prop!(scope, props, "min").unwrap_or(0.0) as f32;
    let max = (get_number_prop!(scope, props, "max").unwrap_or(100.0) as f32).max(min);
    let value = (get_number_prop!(scope, props, "value").unwrap_or(0.0) as f32).clamp(min, max);

    let background = iced_color_from_maybe_v8_string!(get_string_prop!(scope, props, "background"));
    let color = iced_color_from_maybe_v8_string!(get_string_prop!(scope, props, "color"));

    let mut width = get_number_prop!(scope, props, "width").map(|w| w as f32);
    let mut height = get_number_prop!(scope, props, "height").map(|h| h as f32);

    let is_vertical = get_bool_prop!(scope, props, "vertical").unwrap_or(false);

    if is_vertical {
        std::mem::swap(&mut width, &mut height);
    }

    if let Some(width) = width {
        attr_fns.push(Box::new(
            move |progress_bar: ProgressBar| progress_bar.length(width),
        ));
    }

    if let Some(height) = height {
        attr_fns.push(Box::new(
            move |progress_bar: ProgressBar| progress_bar.girth(height),
        ));
    }

    if is_vertical {
        attr_fns.push(Box::new(
            move |progress_bar: ProgressBar| progress_bar.vertical(),
        ));
    }

    ViewFn(Arc::new(move || {
        let progress_bar: ProgressBar = iced::widget::progress_bar(min..=max, value).style(move |theme: &smudgy_theme::Theme| {
            iced::widget::progress_bar::Style {
                background: background.unwrap_or(theme.styles.general.background).into(),
                bar: color.unwrap_or(iced::Color::WHITE).into(),
                border: Default::default(),
            }
        });
        let progress_bar = attr_fns.iter().fold(progress_bar, |progress_bar, attr_fn| attr_fn(progress_bar));
        progress_bar.into()
    }))
}

#[op2]
#[cppgc]
fn op_iced_jsx_create_row(
    #[cppgc] children: &ViewFnVec,
    width: Option<f32>,
    height: Option<f32>,
    spacing: Option<f32>,
    padding: Option<f32>,
) -> ViewFn {
    let children = children.0.take();

    let mut attr_fns: Vec<
        Box<
            dyn Fn(
                iced::widget::Row<'static, Message, smudgy_theme::Theme, iced::Renderer>,
            )
                -> iced::widget::Row<'static, Message, smudgy_theme::Theme, iced::Renderer>,
        >,
    > = Vec::new();

    if let Some(width) = width {
        attr_fns.push(Box::new(
            move |row: iced::widget::Row<'static, Message, smudgy_theme::Theme, iced::Renderer>| {
                row.width(width)
            },
        ));
    }
    if let Some(height) = height {
        attr_fns.push(Box::new(
            move |row: iced::widget::Row<'static, Message, smudgy_theme::Theme, iced::Renderer>| {
                row.height(height)
            },
        ));
    }

    if let Some(spacing) = spacing {
        attr_fns.push(Box::new(
            move |row: iced::widget::Row<'static, Message, smudgy_theme::Theme, iced::Renderer>| {
                row.spacing(spacing)
            },
        ));
    }
    if let Some(padding) = padding {
        attr_fns.push(Box::new(
            move |row: iced::widget::Row<'static, Message, smudgy_theme::Theme, iced::Renderer>| {
                row.padding(padding)
            },
        ));
    }

    ViewFn(Arc::new(move || {
        let row = iced::widget::row(children.iter().map(|c| c.0()));
        let row = attr_fns.iter().fold(row, |row, attr_fn| attr_fn(row));
        row.into()
    }))
}

#[op2]
#[cppgc]
fn op_iced_jsx_create_text<'a>(#[string] content: &str, #[string] color: &str) -> ViewFn {
    let mut attr_fns = Vec::new();

    if color.len() > 0 {
        let color = color_art::Color::from_str(color).unwrap_or_default();
        let color = iced::Color::from_rgba8(
            color.red(),
            color.green(),
            color.blue(),
            color.alpha() as f32,
        );
        attr_fns.push(
            move |text: iced::widget::Text<'static, smudgy_theme::Theme, iced::Renderer>| {
                text.color(color.clone())
            },
        );
    }

    let content = content.to_string();
    ViewFn(Arc::new(move || {
        let text = iced::widget::text(content.to_string());
        let text = attr_fns.iter().fold(text, |text, attr_fn| attr_fn(text));
        text.into()
    }))
}
