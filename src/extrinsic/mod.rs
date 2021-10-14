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

//! Offers macros that build extrinsics for custom runtime modules based on the metadata.
//! Additionally, some predefined extrinsics for common runtime modules are implemented.

pub extern crate codec;
pub extern crate log;

pub mod xt_primitives;

pub type CallIndex = [u8; 2];

#[macro_export]
macro_rules! compose_call {
($node_metadata: expr, $module: expr, $call_name: expr $(, $args: expr) *) => {
        {
            let module = $node_metadata.module_with_calls($module).unwrap().to_owned();

            let call_index = module.calls.get($call_name).unwrap();

            ([module.index, *call_index as u8] $(, ($args)) *)
        }
    };
}

#[macro_export]
macro_rules! compose_extrinsic {
	($api: expr,
	$extra: expr,
	$module: expr,
    $call: expr
	$(, $args: expr) *) => {
		{
            #[allow(unused_imports)] // For when extrinsic does not use Compact
            use codec::Compact;
            use substrate_api_client::extrinsic::xt_primitives::*;

            log::info!("Composing generic extrinsic for module {:?} and call {:?}", $module, $call);
            let call = $crate::compose_call!($api.metadata.clone(), $module, $call $(, ($args)) *);

            UncheckedExtrinsicV4 {
                signature: None,
                function: call.clone(),
            }
		}
    };
}
