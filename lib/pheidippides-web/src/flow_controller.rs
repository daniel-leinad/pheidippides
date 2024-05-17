use std::ops::{ControlFlow, FromResidual, Try};

use http_server::response::Response;


/// This is a convenience enum for providing early return
/// that implements the Try trait so that it can be used with a ? operator
///
/// Think of it this way: when processing a request, you might want to make an early return
/// such as: BadRequest, Redirect to authorization page or InternalServerError
/// if some condition or operation fails.
///
/// However, if that operation/condition does not fail, you would want to continue with the value
/// received from that operation (the value can be a unit)
///
/// This is exactly what this enum represents:
///
/// Value variant for when you need to continue with the value
///
/// HttpResponse variant when you already know the response and want to stop processing the function
#[must_use]
enum HttpResponseFlowController<T> {
    Value(T),
    HttpResponse(Response),
}

impl<T> Try for HttpResponseFlowController<T> {
    type Output = T;
    type Residual = HttpResponseFlowControllerResidual;

    fn from_output(output: Self::Output) -> Self {
        HttpResponseFlowController::Value(output)
    }

    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self {
            HttpResponseFlowController::Value(value) => ControlFlow::Continue(value),
            HttpResponseFlowController::HttpResponse(response) => ControlFlow::Break(HttpResponseFlowControllerResidual(response)),
        }
    }
}

impl<T> FromResidual for HttpResponseFlowController<T> {
    fn from_residual(residual: <Self as Try>::Residual) -> Self {
        Self::HttpResponse(residual.0)
    }
}

struct HttpResponseFlowControllerResidual(Response);

impl FromResidual<HttpResponseFlowControllerResidual> for Response {
    fn from_residual(residual: HttpResponseFlowControllerResidual) -> Self {
        residual.0
    }
}

trait HttpResponseContext {
    type Output;

    fn check(self) -> Option<Self::Output>;
}

trait HttpResponseContextExtension: HttpResponseContext + Sized {
    fn or_bad_request(self) -> HttpResponseFlowController<Self::Output> {
        match self.check() {
            Some(value) => HttpResponseFlowController::Value(value),
            None => HttpResponseFlowController::HttpResponse(Response::BadRequest)
        }
    }

    fn or_server_error(self) -> HttpResponseFlowController<Self::Output> {
        match self.check() {
            Some(value) => HttpResponseFlowController::Value(value),
            None => HttpResponseFlowController::HttpResponse(Response::InternalServerError)
        }
    }
}

impl<T: HttpResponseContext> HttpResponseContextExtension for T {}

impl<T> HttpResponseContext for Option<T> {
    type Output = T;

    fn check(self) -> Option<Self::Output> {
        self
    }
}

impl <T, E> HttpResponseContext for Result<T, E> {
    type Output = T;

    fn check(self) -> Option<Self::Output> {
        match self {
            Ok(value) => Some(value),
            // TODO handle error information here
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use http_server::response::Response;
    use super::HttpResponseContextExtension;

    #[test]
    fn control_flow_works_option() {
        assert!(control_flow_option_some_bad_request().is_empty());
        assert!(control_flow_option_some_server_error().is_empty());
        assert!(control_flow_option_none_bad_request().is_bad_request());
        assert!(control_flow_option_none_server_error().is_internal_server_error());
    }

    fn control_flow_option_some_bad_request() -> Response {
        let _ = Some("value").or_bad_request()?;
        Response::Empty
    }

    fn control_flow_option_some_server_error() -> Response {
        let _ = Some("value").or_server_error()?;
        Response::Empty
    }

    fn control_flow_option_none_bad_request() -> Response {
        let _ = None.or_bad_request()?;
        Response::Empty
    }

    fn control_flow_option_none_server_error() -> Response {
        let _ = None.or_server_error()?;
        Response::Empty
    }

    /////////////////////////////////////////////

    #[test]
    fn control_flow_works_result() {
        assert!(control_flow_result_ok_bad_request().is_empty());
        assert!(control_flow_result_ok_server_error().is_empty());
        assert!(control_flow_result_err_bad_request().is_bad_request());
        assert!(control_flow_result_err_server_error().is_internal_server_error());
    }

    fn control_flow_result_ok_bad_request() -> Response {
        let _ = Ok::<&str, &str>("value").or_bad_request()?;
        Response::Empty
    }

    fn control_flow_result_ok_server_error() -> Response {
        let _ = Ok::<&str, &str>("value").or_server_error()?;
        Response::Empty
    }

    fn control_flow_result_err_bad_request() -> Response {
        let _ = Err::<&str, &str>("error info").or_bad_request()?;
        Response::Empty
    }

    fn control_flow_result_err_server_error() -> Response {
        let _ = Err::<&str, &str>("error info").or_server_error()?;
        Response::Empty
    }
}