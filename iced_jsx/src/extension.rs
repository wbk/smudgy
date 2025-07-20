use std::{borrow::Cow, cell::RefCell, ffi::CStr, rc::Rc, sync::Arc};

use deno_core::{
    GarbageCollected, JsBuffer, OpState, Resource, ResourceId,
    cppgc::Ptr,
    error::AnyError,
    op2, thiserror,
    v8::{self, cppgc::Member},
};

use crate::{IcedJsxRoot};

#[derive(Clone)]
struct ViewFn(pub Arc<dyn Fn() -> iced::Element<'static, (), smudgy_theme::Theme, iced::Renderer>>);
struct ViewFnVec(pub RefCell<Vec<ViewFn>>);

type SmudgyIcedJsxRoot = IcedJsxRoot<'static, (), smudgy_theme::Theme, iced::Renderer>;

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
fn op_iced_jsx_create_column(#[cppgc] children: &ViewFnVec) -> ViewFn {
    let children = children.0.take();
    ViewFn(Arc::new(move || {
        iced::widget::column(children.iter().map(|c| c.0())).into()
    }))
}

#[op2]
#[cppgc]
fn op_iced_jsx_create_row(#[cppgc] children: &ViewFnVec) -> ViewFn {
    let children = children.0.take();
    ViewFn(Arc::new(move || {
        iced::widget::row(children.iter().map(|c| c.0())).into()
    }))
}

#[op2]
#[cppgc]
fn op_iced_jsx_create_text(#[string] content: &str) -> ViewFn {
    let content = content.to_string();
    ViewFn(Arc::new(move || {
        iced::widget::text(content.to_string()).into()
    }))
}

