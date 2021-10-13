/*
   Copyright 2019 Supercomputing Systems AG

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.

*/

pub use client::WsRpcClient;

pub mod client;

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_extrinsic_error_msg(err: RpcClientError) -> String {
        match err {
            RpcClientError::Extrinsic(msg) => msg,
            _ => panic!("Expected extrinsic error"),
        }
    }

    #[test]
    fn extrinsic_status_parsed_correctly() {
        let msg = "{\"jsonrpc\":\"2.0\",\"result\":7185,\"id\":\"3\"}";
        assert_eq!(parse_status(msg).unwrap(), (XtStatus::Unknown, None));

        let msg = "{\"jsonrpc\":\"2.0\",\"method\":\"author_extrinsicUpdate\",\"params\":{\"result\":\"ready\",\"subscription\":7185}}";
        assert_eq!(parse_status(msg).unwrap(), (XtStatus::Ready, None));

        let msg = "{\"jsonrpc\":\"2.0\",\"method\":\"author_extrinsicUpdate\",\"params\":{\"result\":{\"broadcast\":[\"QmfSF4VYWNqNf5KYHpDEdY8Rt1nPUgSkMweDkYzhSWirGY\",\"Qmchhx9SRFeNvqjUK4ZVQ9jH4zhARFkutf9KhbbAmZWBLx\",\"QmQJAqr98EF1X3YfjVKNwQUG9RryqX4Hv33RqGChbz3Ncg\"]},\"subscription\":232}}";
        assert_eq!(
            parse_status(msg).unwrap(),
            (
                XtStatus::Broadcast,
                Some(
                    "[\"QmfSF4VYWNqNf5KYHpDEdY8Rt1nPUgSkMweDkYzhSWirGY\",\"Qmchhx9SRFeNvqjUK4ZVQ9jH4zhARFkutf9KhbbAmZWBLx\",\"QmQJAqr98EF1X3YfjVKNwQUG9RryqX4Hv33RqGChbz3Ncg\"]"
                        .to_string()
                )
            )
        );

        let msg = "{\"jsonrpc\":\"2.0\",\"method\":\"author_extrinsicUpdate\",\"params\":{\"result\":{\"inBlock\":\"0x3104d362365ff5ddb61845e1de441b56c6722e94c1aee362f8aa8ba75bd7a3aa\"},\"subscription\":232}}";
        assert_eq!(
            parse_status(msg).unwrap(),
            (
                XtStatus::InBlock,
                Some(
                    "\"0x3104d362365ff5ddb61845e1de441b56c6722e94c1aee362f8aa8ba75bd7a3aa\""
                        .to_string()
                )
            )
        );

        let msg = "{\"jsonrpc\":\"2.0\",\"method\":\"author_extrinsicUpdate\",\"params\":{\"result\":{\"finalized\":\"0x934385b11c483498e2b5bca64c2e8ef76ad6c74d3372a05595d3a50caf758d52\"},\"subscription\":7185}}";
        assert_eq!(
            parse_status(msg).unwrap(),
            (
                XtStatus::Finalized,
                Some(
                    "\"0x934385b11c483498e2b5bca64c2e8ef76ad6c74d3372a05595d3a50caf758d52\""
                        .to_string()
                )
            )
        );

        let msg = "{\"jsonrpc\":\"2.0\",\"method\":\"author_extrinsicUpdate\",\"params\":{\"result\":\"future\",\"subscription\":2}}";
        assert_eq!(parse_status(msg).unwrap(), (XtStatus::Future, None));

        let msg = "{\"jsonrpc\":\"2.0\",\"error\":{\"code\":-32700,\"message\":\"Parse error\"},\"id\":null}";
        assert_eq!(
            parse_status(msg)
                .map_err(extract_extrinsic_error_msg)
                .unwrap_err(),
            "extrinsic error code -32700: Parse error: ".to_string()
        );

        let msg = "{\"jsonrpc\":\"2.0\",\"error\":{\"code\":1010,\"message\":\"Invalid Transaction\",\"data\":\"Bad Signature\"},\"id\":\"4\"}";
        assert_eq!(
            parse_status(msg)
                .map_err(extract_extrinsic_error_msg)
                .unwrap_err(),
            "extrinsic error code 1010: Invalid Transaction: Bad Signature".to_string()
        );

        let msg = "{\"jsonrpc\":\"2.0\",\"error\":{\"code\":1001,\"message\":\"Extrinsic has invalid format.\"},\"id\":\"0\"}";
        assert_eq!(
            parse_status(msg)
                .map_err(extract_extrinsic_error_msg)
                .unwrap_err(),
            "extrinsic error code 1001: Extrinsic has invalid format.: ".to_string()
        );

        let msg = r#"{"jsonrpc":"2.0","error":{"code":1002,"message":"Verification Error: Execution(Wasmi(Trap(Trap { kind: Unreachable })))","data":"RuntimeApi(\"Execution(Wasmi(Trap(Trap { kind: Unreachable })))\")"},"id":"3"}"#;
        assert_eq!(
            parse_status(msg)
                .map_err(extract_extrinsic_error_msg)
                .unwrap_err(),
            "extrinsic error code 1002: Verification Error: Execution(Wasmi(Trap(Trap { kind: Unreachable }))): RuntimeApi(\"Execution(Wasmi(Trap(Trap { kind: Unreachable })))\")".to_string()
        );
    }
}
