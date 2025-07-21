use std::{borrow::Cow, collections::BTreeMap, marker::PhantomData, sync::{atomic::{AtomicUsize, Ordering}, Arc}};

use arc_swap::ArcSwap;
use deno_core::v8;
use iced::{advanced::graphics::futures::MaybeSend, futures::{channel::mpsc::Sender, SinkExt, Stream}, widget::container, Subscription};
pub struct IcedJsxRoot<'a, Message, Theme, Renderer> {
    inner: Arc<ArcSwap<Inner<'a, Message, Theme, Renderer>>>,
    tx: Arc<tokio::sync::watch::Sender<()>>,
}

impl<'a, Message, Theme, Renderer> std::fmt::Debug for IcedJsxRoot<'a, Message, Theme, Renderer> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "IcedJsxRoot {{ elements: {:?} }}",
            self.inner.load().elements.keys().collect::<Vec<_>>()
        )
    }
}

#[derive(Clone)]
struct Inner<'a, Message, Theme, Renderer> {
    elements: BTreeMap<String, Arc<dyn Fn() -> iced::Element<'a, Message, Theme, Renderer>>>,
}

unsafe impl<'a, Message, Theme, Renderer> Send for IcedJsxRoot<'a, Message, Theme, Renderer> {}
unsafe impl<'a, Message, Theme, Renderer> Sync for IcedJsxRoot<'a, Message, Theme, Renderer> {}

impl<'a, Message, Theme, Renderer> Clone for IcedJsxRoot<'a, Message, Theme, Renderer> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            tx: self.tx.clone()
        }
    }
}

struct HashedArc<T>(pub Arc<T>);

impl<T> std::hash::Hash for HashedArc<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl<'a, Message, Theme, Renderer> IcedJsxRoot<'a, Message, Theme, Renderer>
where
    Message: 'static + Clone,
    Theme: 'a,
    Renderer: 'a,
{
    pub fn new() -> Self {
        let (tx, _) = tokio::sync::watch::channel(());

        Self {
            inner: Arc::new(ArcSwap::from_pointee(Inner {
                elements: BTreeMap::new(),
            })),
            tx: Arc::new(tx),
        }
    }


    pub fn subscription<SubMessage: std::hash::Hash + Copy + Send + 'static>(&self, id: SubMessage) -> Subscription<SubMessage> {
        Subscription::run_with((id, HashedArc(self.tx.clone())), move |(id, ext_tx)| {
            let ext_tx = ext_tx.0.clone();
            let id = *id;
            iced::stream::channel(1, move | mut ui_tx: Sender<SubMessage>| async move {
                let mut rx =  ext_tx.subscribe();
                loop {
                    if let Err(_) = rx.changed().await {
                        break;
                    }
                    if let Err(_) = ui_tx.send(id).await {
                        break;
                    }
                }
            })            
        })
    }

    pub fn insert(
        &self,
        name: &str,
        widget: Arc<dyn Fn() -> iced::Element<'a, Message, Theme, Renderer>>,
    ) {
        let mut elements = self.inner.load().elements.clone();
        elements.insert(name.to_string(), widget.clone());
        self.inner.swap(Arc::new(Inner { elements }));
        self.tx.send(()).ok();
    }

    pub fn remove(&self, name: &str) {
        let mut elements = self.inner.load().elements.clone();
        elements.remove(name);
        self.inner.swap(Arc::new(Inner { elements }));
        self.tx.send(()).ok();
    }
}

impl<'a, Message, Theme, Renderer: iced::advanced::Renderer>
    IcedJsxRoot<'a, Message, Theme, Renderer>
where
    Theme: iced::widget::container::Catalog + 'a,
    Message: 'static,
    Renderer: 'a,
{
    pub fn view(
        &self,
        class: impl Fn() -> Theme::Class<'a>,
    ) -> iced::Element<Message, Theme, Renderer> {
        let inner = self.inner.load();
        if inner.elements.is_empty() {
            iced::widget::column(vec![]).into()
        } else {
            iced::widget::stack(
                inner
                    .elements
                    .values()
                    .map(|e| container(e()).class(class()).into()),
            )
            .into()
        }
    }
}
