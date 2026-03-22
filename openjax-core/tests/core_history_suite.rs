//! Aggregated integration suite for loop detection, history records, and context compression.

#[path = "core_history/m11_context_compression.rs"]
mod context_compression_m11;
#[path = "core_history/m22_history_turn_record.rs"]
mod history_turn_record_m22;
#[path = "core_history/m8_loop_detector.rs"]
mod loop_detector_m8;
