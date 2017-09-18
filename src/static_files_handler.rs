use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::io::ErrorKind::NotFound;
use std::fs;
use std::str::Utf8Error;

use hyper::method::Method::{Get, Head};
use percent_encoding;

use NickelError;
use status::StatusCode;
use request::Request;
use response::Response;
use middleware::{Middleware, MiddlewareResult};

// this should be much simpler after unboxed closures land in Rust.

#[derive(Clone)]
pub struct StaticFilesHandler {
    root_path: PathBuf
}

impl<D> Middleware<D> for StaticFilesHandler {
    fn invoke<'a>(&self, req: &mut Request<D>, res: Response<'a, D>)
            -> MiddlewareResult<'a, D> {
        match req.origin.method {
            Get | Head => {
                match self.extract_path(req) {
                    Some(path) => {
                        let path = Self::percent_decode(path);
                        match path {
                            Ok(path) => self.with_file(Path::new(path.as_ref()), res),
                            Err(e) => Err(NickelError::new(res, e.to_string(), StatusCode::BadRequest))
                        }
                    }
                    None => res.next_middleware()
                }
            },
            _ => res.next_middleware()
        }
    }
}

impl StaticFilesHandler {
    /// Create a new middleware to serve files from within a given root directory.
    /// The file to serve will be determined by combining the requested Url with
    /// the provided root directory.
    ///
    ///
    /// # Examples
    /// ```{rust}
    /// use nickel::{Nickel, StaticFilesHandler};
    /// let mut server = Nickel::new();
    ///
    /// server.utilize(StaticFilesHandler::new("/path/to/serve/"));
    /// ```
    pub fn new<P: AsRef<Path>>(root_path: P) -> StaticFilesHandler {
        StaticFilesHandler {
            root_path: root_path.as_ref().to_path_buf()
        }
    }

    fn extract_path<'a, D>(&self, req: &'a mut Request<D>) -> Option<&'a str> {
        req.path_without_query().map(|path| {
            debug!("{:?} {:?}{:?}", req.origin.method, self.root_path.display(), path);

            match path {
                "/" => "index.html",
                path => &path[1..],
            }
        })
    }

    fn percent_decode<'a>(path: &'a str) -> Result<Cow<'a, str>, Utf8Error> {
        percent_encoding::percent_decode(path.as_bytes()).decode_utf8()
    }

    fn with_file<'a, 'b, D, P>(&self,
                            path: P,
                            res: Response<'a, D>)
            -> MiddlewareResult<'a, D> where P: AsRef<Path> {
        let path = path.as_ref();
        if !safe_path(path) {
            let log_msg = format!("The path '{:?}' was denied access.", path);
            return res.error(StatusCode::BadRequest, log_msg);
        }

        let path = self.root_path.join(path);
        match fs::metadata(&path) {
            Ok(ref attr) if attr.is_file() => res.send_file(&path),
            Err(ref e) if e.kind() != NotFound => {
                debug!("Error getting metadata for file '{:?}': {:?}", path, e);
                res.next_middleware()
            }
            _ => res.next_middleware()
        }
    }
}

/// Block paths from accessing the parent directory
fn safe_path<P: AsRef<Path>>(path: P) -> bool {
    use std::path::Component;

    path.as_ref().components().all(|c| match c {
        // whitelist non-suspicious in case new things get added in future
        Component::CurDir | Component::Normal(_) => true,
        _ => false
    })
}

#[test]
fn bad_paths() {
    let bad_paths = &[
        "foo/bar/../baz/index.html",
        "foo/bar/../baz",
        "../bar/",
        "..",
        "/" // Root path should be handled already
    ];

    for &path in bad_paths {
        assert!(!safe_path(path), "expected {:?} to be suspicious", path);
    }
}

#[test]
fn valid_paths() {
    let good_paths = &[
        "foo/bar/./baz/index.html",
        "foo/bar/./baz",
        "./bar/",
        ".",
        "index.html"
    ];

    for &path in good_paths {
        assert!(safe_path(path), "expected {:?} to not be suspicious", path);
    }
}
