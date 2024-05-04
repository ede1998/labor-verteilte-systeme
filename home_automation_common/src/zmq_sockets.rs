use std::collections::HashMap;

use anyhow::{anyhow, Context as _, Result};

pub use zmq::Error;

use crate::{AnyhowExt, AnyhowZmq};

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
    #[tracing::instrument(skip(self), fields(topic = &*String::from_utf8_lossy(topic.as_ref())))]
    pub fn send<M>(&self, topic: impl AsRef<[u8]>, message: M) -> Result<()>
    where
        M: prost::Message + prost::Name + Default + std::fmt::Debug,
    {
        self.inner
            .send(topic.as_ref(), zmq::SNDMORE)
            .with_context(|| {
                let topic = String::from_utf8_lossy(topic.as_ref());
                format!("Failed to send message {message:?} on topic {topic}")
            })?;

        self.tracing_send(message).with_context(|| {
            let topic = String::from_utf8_lossy(topic.as_ref());
            format!("Failed to send on topic {topic}")
        })
    }
}

impl Subscriber<markers::Linked> {
    /// Block until a message is received on any of the subscribed topics.
    #[tracing::instrument(skip(self))]
    pub fn receive<M>(&self) -> Result<(String, M)>
    where
        M: prost::Message + prost::Name + Default,
    {
        let topic = self
            .inner
            .recv_msg(0)
            .erase_err()
            .and_then(|msg| std::str::from_utf8(&msg).map(ToOwned::to_owned).erase_err())
            .context("Failed to receive topic")?;

        let payload = self.tracing_receive()?;

        Ok((topic, payload.0))
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

impl Requester<markers::Linked> {
    /// Send a message with the REQ-REP pattern.
    #[tracing::instrument(skip(self))]
    pub fn send<M>(&self, message: M) -> Result<()>
    where
        M: prost::Message + prost::Name + std::fmt::Debug,
    {
        let result = self.tracing_send(message);
        trace_result(&result, Direction::Send);
        result
    }

    /// Block until a message is received with the REQ-REP pattern.
    #[tracing::instrument(skip(self))]
    pub fn receive<M>(&self) -> Result<M>
    where
        M: prost::Message + prost::Name + Default,
    {
        let result = self.tracing_receive().map(|(m, _)| m);
        trace_result(&result, Direction::Receive);
        result
    }
}

impl Replier<markers::Linked> {
    /// Send a message with the REQ-REP pattern.
    #[tracing::instrument(skip(self))]
    pub fn send<M>(&self, message: M) -> Result<()>
    where
        M: prost::Message + prost::Name + std::fmt::Debug,
    {
        let result = self.tracing_send(message);
        trace_result(&result, Direction::Send);
        result
    }

    /// Block until a message is received with the REQ-REP pattern.
    // no tracing::instrument here to avoid cycles in span tree
    pub fn receive<M>(&self) -> Result<M>
    where
        M: prost::Message + prost::Name + Default,
    {
        let result = self.tracing_receive().map(|(m, _)| m);
        let _span = tracing::info_span!(stringify!(receive)).entered();
        trace_result(&result, Direction::Receive);
        result
    }
    /// Block until a message is received with the REQ-REP pattern.
    // no tracing::instrument here to avoid cycles in span tree
    pub fn receive_with_ip<M>(&self) -> Result<(M, String)>
    where
        M: prost::Message + prost::Name + Default,
    {
        let result = self.tracing_receive();
        let _span = tracing::info_span!(stringify!(receive)).entered();
        trace_result(&result, Direction::Receive);
        result
    }
}

enum Direction {
    Send,
    Receive,
}

fn trace_result<T: std::fmt::Debug>(result: &Result<T>, direction: Direction) {
    match (direction, result) {
        (Direction::Receive, Err(e)) if e.is_zmq_termination() => {
            tracing::info!(error=%e, "Failed to receive message: {e:#}");
        }
        (Direction::Receive, Err(e)) => {
            tracing::error!(error=%e, "Failed to receive message: {e:#}");
        }
        (Direction::Receive, Ok(m)) => {
            tracing::info!(return=?m, "Received message: {m:?}");
        }
        (Direction::Send, Err(e)) if e.is_zmq_termination() => {
            tracing::info!(error=%e, "Failed to send message: {e:#}");
        }
        (Direction::Send, Err(e)) => {
            tracing::error!(error=%e, "Failed to send message: {e:#}");
        }
        (Direction::Send, Ok(_)) => {
            tracing::info!("Successfully sent message");
        }
    }
}

impl<Kind> Socket<Kind, markers::Linked>
where
    Kind: markers::SocketKind,
{
    /// Receives a message envelope and its contained message of the given type.
    /// Based on the envelope information, the span id is correlated to the remote
    /// span for tracing.
    fn tracing_receive<M>(&self) -> Result<(M, String)>
    where
        M: prost::Message + prost::Name + Default,
    {
        use crate::protobuf::PayloadEnvelope;
        use prost::Message;
        use tracing_opentelemetry::OpenTelemetrySpanExt as _;

        let mut message = self
            .inner
            .recv_msg(0)
            .context("Failed to receive message")?;
        let ip = message
            .gets("Peer-Address")
            .ok_or_else(|| anyhow!("missing remote address"))?
            .to_owned();

        let envelope = PayloadEnvelope::decode(&*message).context("Failed to decode envelope")?;

        let span = tracing::Span::current();
        let parent_cx = opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.extract(&TraceExtractor(&envelope.headers))
        });
        span.set_parent(parent_cx);

        envelope
            .payload
            .ok_or_else(|| anyhow!("Missing payload"))?
            .to_msg()
            .with_context(|| format!("Failed to decode payload {}", std::any::type_name::<M>()))
            .map(|e| (e, ip))
    }

    /// Sends a message envelope that contains the given message.
    fn tracing_send<M>(&self, message: M) -> Result<()>
    where
        M: prost::Message + prost::Name + std::fmt::Debug,
    {
        use crate::protobuf::PayloadEnvelope;
        use prost::Message;
        use tracing_opentelemetry::OpenTelemetrySpanExt as _;

        let span = tracing::Span::current();
        let cx = span.context();
        let mut headers = HashMap::default();
        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut TraceInjector(&mut headers))
        });

        let envelope = PayloadEnvelope {
            headers,
            payload: Some(prost_types::Any::from_msg(&message).unwrap()),
        };
        let buffer = envelope.encode_to_vec();

        self.inner
            .send(buffer, 0)
            .with_context(|| format!("Failed to send message {message:?}"))
    }

    pub fn get_last_endpoint(&self) -> Result<std::net::SocketAddr> {
        let result = self
            .inner
            .get_last_endpoint()
            .context("Failed to get last endpoint")?
            .map_err(|_| anyhow!("Invalid UTF-8"))?;

        result
            .split_once("//")
            .map_or(&*result, |r| r.1)
            .parse()
            .context("Failed to parse endpoint")
    }
}

struct TraceInjector<'a>(&'a mut HashMap<String, String>);

impl<'a> opentelemetry::propagation::Injector for TraceInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.into(), value);
    }
}

struct TraceExtractor<'a>(&'a HashMap<String, String>);

impl<'a> opentelemetry::propagation::Extractor for TraceExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }
}

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
    pub trait ReqRep: SocketKind {}

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
