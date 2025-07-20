use std::{borrow::Cow, collections::BTreeMap, marker::PhantomData, sync::Arc};

use arc_swap::ArcSwap;
use deno_core::v8;
use iced::widget::container;

pub struct IcedJsxRoot<'a, Message, Theme, Renderer> {
    inner: Arc<ArcSwap<Inner<'a, Message, Theme, Renderer>>>,
}

impl <'a, Message, Theme, Renderer>  std::fmt::Debug for IcedJsxRoot<'a, Message, Theme, Renderer> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IcedJsxRoot {{ elements: {:?} }}", self.inner.load().elements.keys().collect::<Vec<_>>())
    }
}

#[derive(Clone)]
struct Inner<'a, Message, Theme, Renderer> {
    elements: BTreeMap<
        String,
        Arc<dyn Fn() -> iced::Element<'a, Message, Theme, Renderer>>,
    >,
}

unsafe impl<'a, Message, Theme, Renderer> Send for IcedJsxRoot<'a, Message, Theme, Renderer> {}
unsafe impl<'a, Message, Theme, Renderer> Sync for IcedJsxRoot<'a, Message, Theme, Renderer> {}

impl<'a, Message, Theme, Renderer> Clone for IcedJsxRoot<'a, Message, Theme, Renderer> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<'a, Message, Theme, Renderer> IcedJsxRoot<'a, Message, Theme, Renderer>
where Message: 'static + Clone,
Theme: 'a,
Renderer: 'a,
{
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ArcSwap::from_pointee(Inner {
                elements: BTreeMap::new(),
            })),
        }
    }

    pub fn insert(&self, name: &str, widget: Arc<dyn Fn() -> iced::Element<'a, Message, Theme, Renderer>>) {
        let mut elements = self.inner.load().elements.clone();
        elements.insert(name.to_string(), widget.clone());
        self.inner.swap(Arc::new(Inner { elements }));
    }

    pub fn remove(&self, name: &str) {
        let mut elements = self.inner.load().elements.clone();
        elements.remove(name);
        self.inner.swap(Arc::new(Inner { elements }));
    }
}

impl<'a, Message, Theme, Renderer: iced::advanced::Renderer> IcedJsxRoot<'a, Message, Theme, Renderer> 
where Theme: iced::widget::container::Catalog + 'a
{    
    pub fn view(&self) -> iced::Element<Message, Theme, Renderer> {
        let inner = self.inner.load();
        if inner.elements.is_empty() {
            iced::widget::column(vec![]).into()
        } else {
            iced::widget::stack(inner.elements.values().map(|e| container(e()).into())).into()
        }
    }
}