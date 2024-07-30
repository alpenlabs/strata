use alpen_vertex_state::client_state::ClientState;
use alpen_vertex_state::operation::ClientUpdateOutput;

use crate::define_table_with_seek_key_codec;
use crate::define_table_without_codec;
use crate::impl_borsh_value_codec;

// Consensus Output Schema and corresponding codecs implementation
define_table_with_seek_key_codec!(
    /// Table to store client state updates.
    (ClientUpdateOutputSchema) u64 => ClientUpdateOutput
);

// Consensus State Schema and corresponding codecs implementation
define_table_with_seek_key_codec!(
    /// Table to store client states.
    (ClientStateSchema) u64 => ClientState
);
