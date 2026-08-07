#ifndef TORCA_NATIVE_H
#define TORCA_NATIVE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct NativeEngineHandle NativeEngineHandle;

uint16_t torca_contract_version(void);
uint8_t *torca_alloc(size_t length);
void torca_free(uint8_t *data, size_t length);

NativeEngineHandle *torca_engine_new(void);
void torca_engine_destroy(NativeEngineHandle *handle);
/* Compatibility close: presentation-local, does not stop the process runtime. */
int32_t torca_engine_close(NativeEngineHandle *handle);
/* Explicit application Quit / process-owner shutdown. */
int32_t torca_process_shutdown(void);

int32_t torca_engine_create_identity(
    NativeEngineHandle *handle,
    const uint8_t *identity_id,
    size_t identity_id_length,
    const uint8_t *display_name,
    size_t display_name_length,
    int64_t at_ms);

int32_t torca_engine_create_pairing(
    NativeEngineHandle *handle,
    const uint8_t *session_id,
    size_t session_id_length);

int32_t torca_engine_join_pairing(
    NativeEngineHandle *handle,
    const uint8_t *session_id,
    size_t session_id_length,
    const uint8_t *code,
    size_t code_length);

int32_t torca_engine_approve_pairing(
    NativeEngineHandle *handle,
    const uint8_t *session_id,
    size_t session_id_length);
int32_t torca_engine_reject_pairing(
    NativeEngineHandle *handle,
    const uint8_t *session_id,
    size_t session_id_length);
int32_t torca_engine_cancel_pairing(
    NativeEngineHandle *handle,
    const uint8_t *session_id,
    size_t session_id_length);

int32_t torca_engine_queue_message(
    NativeEngineHandle *handle,
    const uint8_t *message_id,
    size_t message_id_length,
    const uint8_t *conversation_id,
    size_t conversation_id_length,
    const uint8_t *body,
    size_t body_length,
    int64_t at_ms);

int32_t torca_engine_queue_message_reply(
    NativeEngineHandle *handle,
    const uint8_t *message_id,
    size_t message_id_length,
    const uint8_t *conversation_id,
    size_t conversation_id_length,
    const uint8_t *body,
    size_t body_length,
    const uint8_t *reply_to_message_id,
    size_t reply_to_message_id_length,
    int64_t at_ms);

int32_t torca_engine_mark_conversation_read(
    NativeEngineHandle *handle,
    const uint8_t *conversation_id,
    size_t conversation_id_length);

int32_t torca_engine_queue_attachment(
    NativeEngineHandle *handle,
    const uint8_t *attachment_id,
    size_t attachment_id_length,
    const uint8_t *message_id,
    size_t message_id_length,
    const uint8_t *conversation_id,
    size_t conversation_id_length,
    const uint8_t *source_path,
    size_t source_path_length,
    const uint8_t *name,
    size_t name_length,
    const uint8_t *media_type,
    size_t media_type_length,
    uint64_t size);
int32_t torca_engine_retry_attachment(
    NativeEngineHandle *handle,
    const uint8_t *attachment_id,
    size_t attachment_id_length);
int32_t torca_engine_cancel_attachment(
    NativeEngineHandle *handle,
    const uint8_t *attachment_id,
    size_t attachment_id_length);

int32_t torca_engine_refresh_snapshot(NativeEngineHandle *handle);
int32_t torca_engine_refresh_diagnostics(NativeEngineHandle *handle);

const uint8_t *torca_engine_result_ptr(const NativeEngineHandle *handle);
size_t torca_engine_result_len(const NativeEngineHandle *handle);
const uint8_t *torca_engine_snapshot_ptr(const NativeEngineHandle *handle);
size_t torca_engine_snapshot_len(const NativeEngineHandle *handle);
const uint8_t *torca_engine_diagnostics_ptr(const NativeEngineHandle *handle);
size_t torca_engine_diagnostics_len(const NativeEngineHandle *handle);

#ifdef __cplusplus
}
#endif

#endif