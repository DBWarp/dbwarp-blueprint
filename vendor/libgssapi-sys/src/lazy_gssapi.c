#include <dlfcn.h>
#include <gssapi/gssapi.h>
#include <pthread.h>
#include <stdlib.h>
#include <string.h>

struct gssapi_functions {
    void *handle;
    __typeof__(&gss_acquire_cred) acquire_cred;
    __typeof__(&gss_add_oid_set_member) add_oid_set_member;
    __typeof__(&gss_create_empty_oid_set) create_empty_oid_set;
    __typeof__(&gss_delete_sec_context) delete_sec_context;
    __typeof__(&gss_display_status) display_status;
    __typeof__(&gss_import_name) import_name;
    __typeof__(&gss_init_sec_context) init_sec_context;
    __typeof__(&gss_release_buffer) release_buffer;
    __typeof__(&gss_release_cred) release_cred;
    __typeof__(&gss_release_name) release_name;
    __typeof__(&gss_release_oid_set) release_oid_set;
};

static struct gssapi_functions api;
static pthread_once_t api_once = PTHREAD_ONCE_INIT;

static int load_symbol(void **target, const char *name) {
    *target = dlsym(api.handle, name);
    return *target != NULL;
}

static void load_gssapi(void) {
    static const char *const candidates[] = {
        "libgssapi_krb5.so.2",
        "libgssapi.so.3",
        NULL,
    };
    for (const char *const *candidate = candidates; *candidate != NULL; ++candidate) {
        api.handle = dlopen(*candidate, RTLD_NOW | RTLD_LOCAL);
        if (api.handle != NULL) {
            break;
        }
    }
    if (api.handle == NULL) {
        return;
    }

#define LOAD(field, symbol)                                                        \
    if (!load_symbol((void **)&api.field, symbol)) {                               \
        dlclose(api.handle);                                                        \
        memset(&api, 0, sizeof(api));                                               \
        return;                                                                     \
    }
    LOAD(acquire_cred, "gss_acquire_cred");
    LOAD(add_oid_set_member, "gss_add_oid_set_member");
    LOAD(create_empty_oid_set, "gss_create_empty_oid_set");
    LOAD(delete_sec_context, "gss_delete_sec_context");
    LOAD(display_status, "gss_display_status");
    LOAD(import_name, "gss_import_name");
    LOAD(init_sec_context, "gss_init_sec_context");
    LOAD(release_buffer, "gss_release_buffer");
    LOAD(release_cred, "gss_release_cred");
    LOAD(release_name, "gss_release_name");
    LOAD(release_oid_set, "gss_release_oid_set");
#undef LOAD
}

static int gssapi_available(OM_uint32 *minor_status) {
    pthread_once(&api_once, load_gssapi);
    if (api.handle != NULL) {
        return 1;
    }
    if (minor_status != NULL) {
        *minor_status = 0;
    }
    return 0;
}

int gss_lazy_runtime_available(void) {
    return gssapi_available(NULL);
}

OM_uint32 gss_acquire_cred(
    OM_uint32 *minor_status,
    gss_name_t desired_name,
    OM_uint32 time_req,
    gss_OID_set desired_mechs,
    gss_cred_usage_t cred_usage,
    gss_cred_id_t *output_cred_handle,
    gss_OID_set *actual_mechs,
    OM_uint32 *time_rec) {
    if (!gssapi_available(minor_status)) {
        if (output_cred_handle != NULL) *output_cred_handle = GSS_C_NO_CREDENTIAL;
        if (actual_mechs != NULL) *actual_mechs = GSS_C_NO_OID_SET;
        if (time_rec != NULL) *time_rec = 0;
        return GSS_S_UNAVAILABLE;
    }
    return api.acquire_cred(minor_status, desired_name, time_req, desired_mechs,
                            cred_usage, output_cred_handle, actual_mechs, time_rec);
}

OM_uint32 gss_release_cred(OM_uint32 *minor_status,
                                         gss_cred_id_t *cred_handle) {
    if (!gssapi_available(minor_status)) {
        if (cred_handle != NULL) *cred_handle = GSS_C_NO_CREDENTIAL;
        return GSS_S_COMPLETE;
    }
    return api.release_cred(minor_status, cred_handle);
}

OM_uint32 gss_init_sec_context(
    OM_uint32 *minor_status,
    gss_cred_id_t claimant_cred_handle,
    gss_ctx_id_t *context_handle,
    gss_name_t target_name,
    gss_OID mech_type,
    OM_uint32 req_flags,
    OM_uint32 time_req,
    gss_channel_bindings_t input_chan_bindings,
    gss_buffer_t input_token,
    gss_OID *actual_mech_type,
    gss_buffer_t output_token,
    OM_uint32 *ret_flags,
    OM_uint32 *time_rec) {
    if (!gssapi_available(minor_status)) {
        if (context_handle != NULL) *context_handle = GSS_C_NO_CONTEXT;
        if (actual_mech_type != NULL) *actual_mech_type = GSS_C_NO_OID;
        if (output_token != NULL) *output_token = (gss_buffer_desc)GSS_C_EMPTY_BUFFER;
        if (ret_flags != NULL) *ret_flags = 0;
        if (time_rec != NULL) *time_rec = 0;
        return GSS_S_UNAVAILABLE;
    }
    return api.init_sec_context(minor_status, claimant_cred_handle, context_handle,
                                target_name, mech_type, req_flags, time_req,
                                input_chan_bindings, input_token, actual_mech_type,
                                output_token, ret_flags, time_rec);
}

OM_uint32 gss_delete_sec_context(OM_uint32 *minor_status,
                                                gss_ctx_id_t *context_handle,
                                                gss_buffer_t output_token) {
    if (!gssapi_available(minor_status)) {
        if (context_handle != NULL) *context_handle = GSS_C_NO_CONTEXT;
        if (output_token != NULL) *output_token = (gss_buffer_desc)GSS_C_EMPTY_BUFFER;
        return GSS_S_COMPLETE;
    }
    return api.delete_sec_context(minor_status, context_handle, output_token);
}

OM_uint32 gss_display_status(
    OM_uint32 *minor_status,
    OM_uint32 status_value,
    int status_type,
    gss_OID mech_type,
    OM_uint32 *message_context,
    gss_buffer_t status_string) {
    if (gssapi_available(minor_status)) {
        return api.display_status(minor_status, status_value, status_type, mech_type,
                                  message_context, status_string);
    }
    static const char message[] = "GSSAPI runtime library is unavailable";
    char *copy = malloc(sizeof(message) - 1);
    if (copy == NULL) {
        return GSS_S_FAILURE;
    }
    memcpy(copy, message, sizeof(message) - 1);
    if (status_string != NULL) {
        status_string->length = sizeof(message) - 1;
        status_string->value = copy;
    } else {
        free(copy);
    }
    if (message_context != NULL) *message_context = 0;
    if (minor_status != NULL) *minor_status = 0;
    return GSS_S_COMPLETE;
}

OM_uint32 gss_import_name(OM_uint32 *minor_status,
                                        gss_buffer_t input_name_buffer,
                                        gss_OID input_name_type,
                                        gss_name_t *output_name) {
    if (!gssapi_available(minor_status)) {
        if (output_name != NULL) *output_name = GSS_C_NO_NAME;
        return GSS_S_UNAVAILABLE;
    }
    return api.import_name(minor_status, input_name_buffer, input_name_type, output_name);
}

OM_uint32 gss_release_name(OM_uint32 *minor_status,
                                         gss_name_t *input_name) {
    if (!gssapi_available(minor_status)) {
        if (input_name != NULL) *input_name = GSS_C_NO_NAME;
        return GSS_S_COMPLETE;
    }
    return api.release_name(minor_status, input_name);
}

OM_uint32 gss_release_buffer(OM_uint32 *minor_status,
                                           gss_buffer_t buffer) {
    if (!gssapi_available(minor_status)) {
        if (buffer != NULL && buffer->value != NULL) free(buffer->value);
        if (buffer != NULL) *buffer = (gss_buffer_desc)GSS_C_EMPTY_BUFFER;
        return GSS_S_COMPLETE;
    }
    return api.release_buffer(minor_status, buffer);
}

OM_uint32 gss_release_oid_set(OM_uint32 *minor_status,
                                            gss_OID_set *set) {
    if (!gssapi_available(minor_status)) {
        if (set != NULL) *set = GSS_C_NO_OID_SET;
        return GSS_S_COMPLETE;
    }
    return api.release_oid_set(minor_status, set);
}

OM_uint32 gss_create_empty_oid_set(OM_uint32 *minor_status,
                                                  gss_OID_set *oid_set) {
    if (!gssapi_available(minor_status)) {
        if (oid_set != NULL) *oid_set = GSS_C_NO_OID_SET;
        return GSS_S_UNAVAILABLE;
    }
    return api.create_empty_oid_set(minor_status, oid_set);
}

OM_uint32 gss_add_oid_set_member(OM_uint32 *minor_status,
                                                gss_OID member_oid,
                                                gss_OID_set *oid_set) {
    if (!gssapi_available(minor_status)) {
        return GSS_S_UNAVAILABLE;
    }
    return api.add_oid_set_member(minor_status, member_oid, oid_set);
}
