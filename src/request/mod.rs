//! Iron's HTTP Request representation and associated methods.

use std::io::{self, Read};
use std::net::SocketAddr;
use std::fmt::{self, Debug};
use std::mem::transmute;

use hyper::uri::RequestUri::{AbsoluteUri, AbsolutePath};
use hyper::net::NetworkStream;
use hyper::http::h1::HttpReader;

use typemap::TypeMap;
use plugin::Extensible;
use method::Method;

pub use hyper::server::request::Request as HttpRequest;
use hyper::buffer;

pub use self::url::Url;

use {Protocol, Plugin, Headers, Set, headers};

mod url;

/// The `Request` given to all `Middleware`.
///
/// Stores all the properties of the client's request plus
/// an `TypeMap` for data communication between middleware.
pub struct Request<'a> {
    /// The requested URL.
    pub url: Url,

    /// The originating address of the request.
    pub remote_addr: SocketAddr,

    /// The local address of the request.
    pub local_addr: SocketAddr,

    /// The request headers.
    pub headers: Headers,

    /// The request body as a reader.
    pub body: Body<'a>,

    /// The request method.
    pub method: Method,

    /// Extensible storage for data passed between middleware.
    pub extensions: TypeMap
}

impl<'a> Debug for Request<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(writeln!(f, "Request {{"));

        try!(writeln!(f, "    url: {:?}", self.url));
        try!(writeln!(f, "    method: {:?}", self.method));
        try!(writeln!(f, "    remote_addr: {:?}", self.remote_addr));
        try!(writeln!(f, "    local_addr: {:?}", self.local_addr));

        try!(write!(f, "}}"));
        Ok(())
    }
}

impl<'a, 'b> Request<'a> {
    /// Create a request from an HttpRequest.
    ///
    /// This constructor consumes the HttpRequest.
    pub fn from_http(req: HttpRequest<'a, 'b>, local_addr: SocketAddr, protocol: &Protocol)
                     -> Result<Request<'a>, String> {
        let (addr, method, headers, uri, _, reader) = req.deconstruct();

        let url = match uri {
            AbsoluteUri(ref url) => {
                match Url::from_generic_url(url.clone()) {
                    Ok(url) => url,
                    Err(e) => return Err(e)
                }
            },

            AbsolutePath(ref path) => {
                // Attempt to prepend the Host header (mandatory in HTTP/1.1)
                let url_string = match headers.get::<headers::Host>() {
                    Some(ref host) => {
                        format!("{}://{}:{}{}", protocol.name(), host.hostname, local_addr.port(),
                                path)
                    },
                    None => return Err("No host specified in request".into())
                };

                match Url::parse(&url_string) {
                    Ok(url) => url,
                    Err(e) => return Err(format!("Couldn't parse requested URL: {}", e))
                }
            },
            _ => return Err("Unsupported request URI".into())
        };

        Ok(Request {
            url: url,
            remote_addr: addr,
            local_addr: local_addr,
            headers: headers,
            body: Body::new(reader),
            method: method,
            extensions: TypeMap::new()
        })
    }
}

/// The body of an Iron request,
#[derive(Debug)]
pub struct Body<'a>(Box<HttpReader<&'a mut Read>>);

impl<'a> Body<'a> {
    /// Create a new reader for use in an Iron request from a hyper HttpReader.
    pub fn new<'b>(reader: HttpReader<&'a mut buffer::BufReader<&'b mut NetworkStream>>) -> Body<'a> {
        let transmuted: Box<HttpReader<&mut Read>> = unsafe {
            transmute(box reader)
        };
        Body(transmuted)
    }
    pub fn from_reader(reader: &'a mut Read, len: Option<u64>, chunked: bool) -> Body<'a> {
        let http_reader = if len.is_some() && ! chunked {
            HttpReader::SizedReader(reader, len.unwrap())
        }
        else if chunked { 
            HttpReader::ChunkedReader(reader, len)
        }
        else if ! len.is_some() && ! chunked {
            HttpReader::EofReader(reader)
        }
        else {
            HttpReader::EmptyReader(reader)
        };
        Body(box http_reader)
    }
}

impl<'a> Read for Body<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

// Allow plugins to attach to requests.
impl<'a> Extensible for Request<'a> {
    fn extensions(&self) -> &TypeMap {
        &self.extensions
    }

    fn extensions_mut(&mut self) -> &mut TypeMap {
        &mut self.extensions
    }
}

impl<'a> Plugin for Request<'a> {}
impl<'a> Set for Request<'a> {}
