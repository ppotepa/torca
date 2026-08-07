#ifndef TORCA_NATIVE_H
#define TORCA_NATIVE_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct NativeEngineHandle NativeEngineHandle;
uint16_t torca_contract_version(void); uint64_t torca_max_attachment_bytes(void); uint8_t *torca_alloc(size_t length); void torca_free(uint8_t *data,size_t length);
NativeEngineHandle *torca_engine_new(void); void torca_engine_destroy(NativeEngineHandle *handle); int32_t torca_engine_close(NativeEngineHandle *handle); int32_t torca_process_shutdown(void);
int32_t torca_engine_create_identity_intent(NativeEngineHandle*,const uint8_t*,size_t);
int32_t torca_engine_create_pairing_intent(NativeEngineHandle*);
int32_t torca_engine_join_pairing_intent(NativeEngineHandle*,const uint8_t*,size_t);
int32_t torca_engine_queue_message_intent(NativeEngineHandle*,const uint8_t*,size_t,const uint8_t*,size_t,const uint8_t*,size_t);
int32_t torca_engine_retry_message_intent(NativeEngineHandle*,const uint8_t*,size_t);
int32_t torca_engine_mark_conversation_read_intent(NativeEngineHandle*,const uint8_t*,size_t,uint8_t);
int32_t torca_engine_queue_attachment_intent(NativeEngineHandle*,const uint8_t*,size_t,const uint8_t*,size_t,const uint8_t*,size_t,const uint8_t*,size_t,uint64_t);
int32_t torca_engine_create_identity(NativeEngineHandle*,const uint8_t*,size_t,const uint8_t*,size_t,int64_t);
int32_t torca_engine_create_pairing(NativeEngineHandle*,const uint8_t*,size_t); int32_t torca_engine_join_pairing(NativeEngineHandle*,const uint8_t*,size_t,const uint8_t*,size_t); int32_t torca_engine_approve_pairing(NativeEngineHandle*,const uint8_t*,size_t); int32_t torca_engine_reject_pairing(NativeEngineHandle*,const uint8_t*,size_t); int32_t torca_engine_cancel_pairing(NativeEngineHandle*,const uint8_t*,size_t);
int32_t torca_engine_rename_contact(NativeEngineHandle*,const uint8_t*,size_t,const uint8_t*,size_t); int32_t torca_engine_verify_contact(NativeEngineHandle*,const uint8_t*,size_t); int32_t torca_engine_reset_contact_verification(NativeEngineHandle*,const uint8_t*,size_t); int32_t torca_engine_block_contact(NativeEngineHandle*,const uint8_t*,size_t); int32_t torca_engine_unblock_contact(NativeEngineHandle*,const uint8_t*,size_t); int32_t torca_engine_remove_contact(NativeEngineHandle*,const uint8_t*,size_t); int32_t torca_engine_clear_conversation_history(NativeEngineHandle*,const uint8_t*,size_t);
int32_t torca_engine_queue_message(NativeEngineHandle*,const uint8_t*,size_t,const uint8_t*,size_t,const uint8_t*,size_t,int64_t); int32_t torca_engine_queue_message_reply(NativeEngineHandle*,const uint8_t*,size_t,const uint8_t*,size_t,const uint8_t*,size_t,const uint8_t*,size_t,int64_t); int32_t torca_engine_retry_message(NativeEngineHandle*,const uint8_t*,size_t,int64_t); int32_t torca_engine_mark_conversation_read(NativeEngineHandle*,const uint8_t*,size_t);
int32_t torca_engine_queue_attachment(NativeEngineHandle*,const uint8_t*,size_t,const uint8_t*,size_t,const uint8_t*,size_t,const uint8_t*,size_t,const uint8_t*,size_t,const uint8_t*,size_t,uint64_t); int32_t torca_engine_retry_attachment(NativeEngineHandle*,const uint8_t*,size_t); int32_t torca_engine_cancel_attachment(NativeEngineHandle*,const uint8_t*,size_t); int32_t torca_engine_export_attachment(NativeEngineHandle*,const uint8_t*,size_t,const uint8_t*,size_t);
int32_t torca_engine_refresh_snapshot(NativeEngineHandle*); int32_t torca_engine_refresh_diagnostics(NativeEngineHandle*);
const uint8_t *torca_engine_result_ptr(const NativeEngineHandle*); size_t torca_engine_result_len(const NativeEngineHandle*); const uint8_t *torca_engine_snapshot_ptr(const NativeEngineHandle*); size_t torca_engine_snapshot_len(const NativeEngineHandle*); const uint8_t *torca_engine_diagnostics_ptr(const NativeEngineHandle*); size_t torca_engine_diagnostics_len(const NativeEngineHandle*);
#ifdef __cplusplus
}
#endif
#endif
