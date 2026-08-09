#ifndef TORCA_NATIVE_H
#define TORCA_NATIVE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct TorcaRuntimeHandle TorcaRuntimeHandle;

const uint8_t *torca_runtime_metadata_ptr(void);
size_t torca_runtime_metadata_len(void);
uint8_t *torca_alloc(size_t length);
void torca_free(uint8_t *data, size_t length);
TorcaRuntimeHandle *torca_runtime_acquire(void);
void torca_runtime_release(TorcaRuntimeHandle *handle);
int32_t torca_runtime_invoke(TorcaRuntimeHandle *handle,
    const uint8_t *request, size_t request_length, uint32_t timeout_ms);
const uint8_t *torca_runtime_response_ptr(const TorcaRuntimeHandle *handle);
size_t torca_runtime_response_len(const TorcaRuntimeHandle *handle);
int32_t torca_runtime_shutdown(uint32_t timeout_ms);

#ifdef __cplusplus
}
#endif
#endif
