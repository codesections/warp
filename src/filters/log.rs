//! Logger Filters

use std::marker::PhantomData;
use std::time::Instant;

use futures::Future;
use http::StatusCode;

use ::filter::{Filter, FilterClone, One};
use ::never::Never;
use ::reject::{CombineRejection, Reject};
use ::reply::{Reply, ReplySealed};
use ::route;

use self::internal::Logged;

/// Create a decorating filter with the specified `name` as the `target`.
///
/// This uses the default access logging format, and log records produced
/// will have their `target` set to `name`.
///
/// # Example
///
/// ```
/// use warp::Filter;
///
/// // If using something like `env_logger` or `pretty_env_logger`,
/// // view logs by setting `RUST_LOG=example::api`.
/// let log = warp::log("example::api");
/// let route = log.decorate(
///     warp::any().map(warp::reply)
/// );
/// ```
pub fn log(name: &'static str) -> Log<impl Fn(Info) + Copy> {
    let func = move |info: Info| {
        route::with(|route| {
            // TODO:
            // - remote_addr
            // - response content length
            // - date
            info!(
                target: name,
                "\"{} {} {:?}\" {} {:?}",
                route.method(),
                route.full_path(),
                route.version(),
                info.status.as_u16(),
                info.start.elapsed(),
            );
        });
    };
    Log {
        func,
    }
}

// TODO:
// pub fn custom(impl Fn(Info)) -> Log

/// Decorates a [`Filter`](::Filter) to log requests and responses.
#[derive(Clone, Copy, Debug)]
pub struct Log<F> {
    func: F,
}

/// Information about the request/response that can be used to prepare log lines.
#[allow(missing_debug_implementations)]
pub struct Info<'a> {
    start: Instant,
    status: StatusCode,
    // This struct will eventually hold a `&'a Route` and `&'a Response`,
    // so use a marker so there can be a lifetime in the struct definition.
    _marker: PhantomData<&'a ()>,
}

impl<FN> Log<FN>
where
    FN: Fn(Info) + Clone + Send,
{
    /// Decorates a [`Filter`](::Filter) to log requests and responses handled by inner.
    pub fn decorate<F>(&self, inner: F) -> impl FilterClone<
        Extract=One<Logged>,
        Error=<F::Error as CombineRejection<Never>>::Rejection
    >
    where
        F: Filter + Clone + Send,
        F::Extract: Reply,
        F::Error: CombineRejection<Never> + Reject,
    {
        let func = self.func.clone();
        ::filters::any::any()
            .and_then(move || {
                let start = Instant::now();
                let func = func.clone();
                inner
                    .filter()
                    .then(move |result| {
                        let (result, status) = match result {
                            Ok(rep) => {
                                let resp = rep.into_response();
                                let status = resp.status();
                                (Ok(Logged(resp)), status)
                            },
                            Err(reject) => {
                                let status = reject.status();
                                (Err(reject), status)
                            }
                        };
                        func(Info {
                            start,
                            status,
                            _marker: PhantomData,
                        });
                        result
                    })
            })
    }
}

mod internal {
    use ::reply::{ReplySealed, Response};

    #[allow(missing_debug_implementations)]
    pub struct Logged(pub(super) Response);

    impl ReplySealed for Logged {
        #[inline]
        fn into_response(self) -> Response {
            self.0
        }
    }
}