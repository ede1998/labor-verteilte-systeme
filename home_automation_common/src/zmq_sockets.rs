use anyhow::{Context as _, Result};

pub use zmq::Error;

use crate::AnyhowExt;

/// Handle for a ØMQ context, used to create sockets.
///
/// It is thread safe, and can be safely cloned and shared. Each clone
/// references the same underlying C context. Internally, an `Arc` is
/// used to implement this in a thread-safe way.
///
/// Also note that this binding deviates from the C API in that each
/// socket created from a context initially owns a clone of that
/// context. This reference is kept to avoid a potential deadlock
/// situation that would otherwise occur:
///
/// Destroying the underlying C context is an operation which
/// blocks waiting for all sockets created from it to be closed
/// first. If one of the sockets belongs to thread issuing the
/// destroy operation, you have established a deadlock.
///
/// You can still deadlock yourself (or intentionally close sockets in
/// other threads, see `zmq_ctx_destroy`(3)) by explicitly calling
/// `Context::destroy`.
#[derive(Clone, Default)]
pub struct Context(zmq::Context);

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let threads = self.get_io_threads().ok();
        f.debug_struct(stringify!(Context))
            .field("io_threads", &threads)
            .finish()
    }
}

impl Context {
    /// Create a new reference-counted context handle.
    pub fn new() -> Context {
        Self(zmq::Context::new())
    }

    /// Get the size of the ØMQ thread pool to handle I/O operations.
    pub fn get_io_threads(&self) -> Result<i32> {
        self.0
            .get_io_threads()
            .context("Failed to read I/O thread count")
    }

    /// Set the size of the ØMQ thread pool to handle I/O operations.
    pub fn set_io_threads(&self, value: i32) -> Result<()> {
        self.0
            .set_io_threads(value)
            .context("Failed to set I/O thread count")
    }

    /// Try to destroy the context. This is different than the destructor; the
    /// destructor will loop when zmq_ctx_term returns EINTR.
    pub fn destroy(&mut self) -> Result<()> {
        self.0.destroy().context("Failed to destroy ZMQ context")
    }
}

/// Represents a socket.
///
/// The generic parameter `Kind` represents the type of ØMQ socket. It can be any of:
/// - [`Publisher`][markers::Publisher] = `PUB`
/// - [`Subscriber`][markers::Subscriber] = `SUB`
/// - [`Requester`][markers::Requester] = `REQ`
/// - [`Replier`][markers::Replier] = `REP`
///
/// The generic parameter `LinkState` is either [`Detached`][markers::Detached] or
/// [`Linked`][markers::Linked] to represent a socket that is bound or connected to
/// an endpoint or one that was not yet bound or connected.
pub struct Socket<Kind, LinkState> {
    inner: zmq::Socket,
    kind: Kind,
    link_state: LinkState,
}

pub type Publisher<LinkState = markers::Detached> = Socket<markers::Publisher, LinkState>;
pub type Subscriber<LinkState = markers::Detached> = Socket<markers::Subscriber, LinkState>;
pub type Requester<LinkState = markers::Detached> = Socket<markers::Requester, LinkState>;
pub type Replier<LinkState = markers::Detached> = Socket<markers::Replier, LinkState>;

impl<Kind, LinkState> std::fmt::Debug for Socket<Kind, LinkState>
where
    Kind: std::fmt::Debug,
    LinkState: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Socket")
            .field("kind", &self.kind)
            .field("link_state", &self.link_state)
            .finish()
    }
}

impl<Kind> Socket<Kind, markers::Detached>
where
    Kind: markers::SocketKind,
{
    /// Create a new socket.
    ///
    /// Note that the returned socket keeps a an `Arc` reference to
    /// the context it was created from, and will keep that context
    /// from being dropped while being live.
    pub fn new(ctx: &Context) -> Result<Self> {
        ctx.0
            .socket(Kind::KIND)
            .map(|inner| Self {
                inner,
                kind: Kind::default(),
                link_state: markers::Detached,
            })
            .with_context(|| format!("Failed to create {:?} socket", Kind::default()))
    }
}

impl<Kind> Socket<Kind, markers::Detached> {
    /// Connect a socket.
    pub fn connect(self, endpoint: &str) -> Result<Socket<Kind, markers::Linked>> {
        self.inner
            .connect(endpoint)
            .with_context(|| format!("Failed to connect to {endpoint}"))?;
        Ok(Socket {
            inner: self.inner,
            link_state: markers::Linked,
            kind: self.kind,
        })
    }

    /// Accept connections on a socket.
    pub fn bind(self, endpoint: &str) -> Result<Socket<Kind, markers::Linked>> {
        self.inner
            .bind(endpoint)
            .with_context(|| format!("Failed to bind to {endpoint}"))?;
        Ok(Socket {
            inner: self.inner,
            link_state: markers::Linked,
            kind: self.kind,
        })
    }
}

impl Publisher<markers::Linked> {
    /// Publish the given message on the given topic.
    pub fn send<M>(&self, topic: impl AsRef<[u8]>, message: M) -> Result<()>
    where
        M: prost::Message + Default,
    {
        let topic_msg: zmq::Message = topic.as_ref().into();
        let buffer_msg = message.encode_to_vec().into();
        self.inner
            .send_multipart([topic_msg, buffer_msg], 0)
            .with_context(|| {
                let topic = String::from_utf8_lossy(topic.as_ref());
                format!("Failed to send message {message:?} on topic {topic}")
            })
    }
}

impl Subscriber<markers::Linked> {
    /// Block until a message is received on any of the subscribed topics.
    pub fn receive<M>(&self) -> Result<(String, M)>
    where
        M: prost::Message + Default,
    {
        let topic = self
            .inner
            .recv_msg(0)
            .erase_err()
            .and_then(|msg| std::str::from_utf8(&msg).map(ToOwned::to_owned).erase_err())
            .context("Failed to receive topic")?;
        let bytes = self
            .inner
            .recv_msg(0)
            .context("Failed to receive payload")?;
        let payload = M::decode(&*bytes)
            .with_context(|| format!("Failed to decode payload {}", std::any::type_name::<M>()))?;

        Ok((topic, payload))
    }
}

impl<LinkState> Subscriber<LinkState> {
    /// Subscribe to the given topic.
    pub fn subscribe(&self, topic: impl AsRef<[u8]>) -> Result<()> {
        self.inner.set_subscribe(topic.as_ref()).with_context(|| {
            let topic = String::from_utf8_lossy(topic.as_ref());
            format!("Failed to subscribe to {topic}")
        })
    }

    /// Unsubscribe from the given topic.
    pub fn unsubscribe(&self, topic: impl AsRef<[u8]>) -> Result<()> {
        self.inner.set_unsubscribe(topic.as_ref()).with_context(|| {
            let topic = String::from_utf8_lossy(topic.as_ref());
            format!("Failed to subscribe to {topic}")
        })
    }
}

impl<Kind> Socket<Kind, markers::Linked>
where
    Kind: markers::ReqRep,
{
    /// Send a message with the REQ-REP pattern.
    pub fn send<M>(&self, message: M) -> Result<()>
    where
        M: prost::Message + std::fmt::Debug,
    {
        let buffer = message.encode_to_vec();
        self.inner
            .send(buffer, 0)
            .with_context(|| format!("Failed to send message {message:?}"))
    }

    /// Block until a message is received with the REQ-REP pattern.
    pub fn receive<M>(&self) -> Result<M>
    where
        M: prost::Message + Default,
    {
        let bytes = self
            .inner
            .recv_msg(0)
            .context("Failed to receive payload")?;
        M::decode(&*bytes)
            .with_context(|| format!("Failed to decode payload {}", std::any::type_name::<M>()))
    }
}

impl Replier<markers::Linked> {}

pub mod markers {
    #[derive(Debug, Default, Clone, Copy)]
    pub struct Linked;

    #[derive(Debug, Default, Clone, Copy)]
    pub struct Detached;

    #[derive(Debug, Default, Clone, Copy)]
    pub struct Publisher;

    #[derive(Debug, Default, Clone, Copy)]
    pub struct Subscriber;

    #[derive(Debug, Default, Clone, Copy)]
    pub struct Requester;

    #[derive(Debug, Default, Clone, Copy)]
    pub struct Replier;

    mod sealed {
        pub trait Seal {}

        impl Seal for super::Subscriber {}
        impl Seal for super::Publisher {}
        impl Seal for super::Requester {}
        impl Seal for super::Replier {}
    }

    #[doc(hidden)]
    pub trait ReqRep: sealed::Seal {}

    impl ReqRep for Requester {}
    impl ReqRep for Replier {}

    #[doc(hidden)]
    pub trait SocketKind: Default + std::fmt::Debug + sealed::Seal {
        const KIND: zmq::SocketType;
    }

    impl SocketKind for Publisher {
        const KIND: zmq::SocketType = zmq::SocketType::PUB;
    }

    impl SocketKind for Subscriber {
        const KIND: zmq::SocketType = zmq::SocketType::SUB;
    }

    impl SocketKind for Requester {
        const KIND: zmq::SocketType = zmq::SocketType::REQ;
    }

    impl SocketKind for Replier {
        const KIND: zmq::SocketType = zmq::SocketType::REP;
    }
}
