#ifndef TORCA_NATIVE_H
#define TORCA_NATIVE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct NativeEngineRuntime NativeEngineRuntime;

uint16_t torca_contract_version(void);
uint8_t *torca_alloc(size_t length);
void torca_free(uint8_t *data, size_t length);

NativeEngineRuntime *torca_engine_new(void);
void torca_engine_destroy(NativeEngineRuntime *handle);
int32_t torca_engine_close(NativeEngineRuntime *handle);

int32_t torca_engine_create_identity(
    NativeEngineRuntime *handle,
    const uint8_t *identity_id,
    size_t identity_id_length,
    const uint8_t *display_name,
    size_t display_name_length,
    int64_t at_ms);

int32_t torca_engine_start_pairing(
    NativeEngineRuntime *handle,
    const uint8_t *session_id,
    size_t session_id_length,
    const uint8_t *code,
    size_t code_length,
    int64_t expires_at_ms);

int32_t torca_engine_queue_message(
    NativeEngineRuntime *handle,
    const uint8_t *message_id,
    size_t message_id_length,
    const uint8_t *conversation_id,
    size_t conversation_id_length,
    const uint8_t *body,
    size_t body_length,
    int64_t at_ms);

int32_t torca_engine_refresh_snapshot(NativeEngineRuntime *handle);

const uint8_t *torca_engine_result_ptr(const NativeEngineRuntime *handle);
size_t torca_engine_result_len(const NativeEngineRuntime *handle);
const uint8_t *torca_engine_snapshot_ptr(const NativeEngineRuntime *handle);
size_t torca_engine_snapshot_len(const NativeEngineRuntime *handle);

#ifdef __cplusplus
}
#endif

#endif
