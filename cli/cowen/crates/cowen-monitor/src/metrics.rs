#[macro_export]
macro_rules! counter {
    ($name:expr, $help:expr) => {{
        let opts = prometheus::Opts::new($name, $help);
        let counter = prometheus::Counter::with_opts(opts).unwrap();
        if let Err(e) = prometheus::register(Box::new(counter.clone())) {
             if let prometheus::Error::AlreadyReg = e {
                 // Ignore: Already registered
             } else {
                 panic!("Failed to register counter: {}", e);
             }
        }
        counter
    }};
}

#[macro_export]
macro_rules! gauge {
    ($name:expr, $help:expr) => {{
        let opts = prometheus::Opts::new($name, $help);
        let gauge = prometheus::Gauge::with_opts(opts).unwrap();
        if let Err(e) = prometheus::register(Box::new(gauge.clone())) {
             if let prometheus::Error::AlreadyReg = e {
                 // Ignore: Already registered
             } else {
                 panic!("Failed to register gauge: {}", e);
             }
        }
        gauge
    }};
}
