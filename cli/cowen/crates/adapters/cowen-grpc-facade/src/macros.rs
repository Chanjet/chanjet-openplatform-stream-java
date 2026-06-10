#[macro_export]
macro_rules! grpc_forward {
    ($self:expr, $capability:ident, $method:ident, $request:expr) => {{
        let claims = $request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match $self.capabilities.$capability.$method(claims.as_ref(), $request.into_inner()).await {
            Ok(resp) => Ok(tonic::Response::new(resp)),
            Err(e) => Err(tonic::Status::internal(e.to_string())),
        }
    }};
}
