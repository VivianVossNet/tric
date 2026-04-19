/* Copyright 2025-2026 Vivian Voss. Licensed under the BSD 3-Clause License. */
/* SPDX-License-Identifier: BSD-3-Clause */
/* Scope: TRIC+ Tcl client — loadable C extension exposing tric::* commands. */

#include "tric.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <tcl.h>

typedef struct {
    TricConnection handle;
    int owned;
} TricHandle;

static Tcl_HashTable g_handles;
static int g_next_id = 1;

static TricHandle *lookup_handle(Tcl_Interp *interp, const char *token) {
    Tcl_HashEntry *entry = Tcl_FindHashEntry(&g_handles, token);
    if (entry == NULL) {
        Tcl_SetObjResult(interp, Tcl_NewStringObj("invalid handle", -1));
        return NULL;
    }
    return (TricHandle *)Tcl_GetHashValue(entry);
}

static int connect_cmd(ClientData cd, Tcl_Interp *interp, int objc, Tcl_Obj *const objv[]) {
    (void)cd;
    if (objc != 2) {
        Tcl_WrongNumArgs(interp, 1, objv, "socketPath");
        return TCL_ERROR;
    }
    const char *path = Tcl_GetString(objv[1]);
    TricHandle *th = malloc(sizeof(TricHandle));
    if (th == NULL) {
        Tcl_SetObjResult(interp, Tcl_NewStringObj("out of memory", -1));
        return TCL_ERROR;
    }
    th->handle = create_connection(path);
    th->owned = th->handle.socket_fd >= 0;
    if (!th->owned) {
        free(th);
        Tcl_SetObjResult(interp, Tcl_NewStringObj("connect failed", -1));
        return TCL_ERROR;
    }
    char token[32];
    snprintf(token, sizeof(token), "trich%d", g_next_id++);
    int new_entry = 0;
    Tcl_HashEntry *entry = Tcl_CreateHashEntry(&g_handles, token, &new_entry);
    Tcl_SetHashValue(entry, th);
    Tcl_SetObjResult(interp, Tcl_NewStringObj(token, -1));
    return TCL_OK;
}

static int disconnect_cmd(ClientData cd, Tcl_Interp *interp, int objc, Tcl_Obj *const objv[]) {
    (void)cd;
    if (objc != 2) {
        Tcl_WrongNumArgs(interp, 1, objv, "handle");
        return TCL_ERROR;
    }
    const char *token = Tcl_GetString(objv[1]);
    Tcl_HashEntry *entry = Tcl_FindHashEntry(&g_handles, token);
    if (entry == NULL) {
        return TCL_OK;
    }
    TricHandle *th = (TricHandle *)Tcl_GetHashValue(entry);
    if (th->owned) {
        delete_connection(&th->handle);
    }
    free(th);
    Tcl_DeleteHashEntry(entry);
    return TCL_OK;
}

static int valid_cmd(ClientData cd, Tcl_Interp *interp, int objc, Tcl_Obj *const objv[]) {
    (void)cd;
    if (objc != 2) {
        Tcl_WrongNumArgs(interp, 1, objv, "handle");
        return TCL_ERROR;
    }
    TricHandle *th = lookup_handle(interp, Tcl_GetString(objv[1]));
    if (th == NULL) return TCL_ERROR;
    Tcl_SetObjResult(interp, Tcl_NewBooleanObj(th->owned && check_connection(&th->handle) != 0));
    return TCL_OK;
}

static int read_cmd(ClientData cd, Tcl_Interp *interp, int objc, Tcl_Obj *const objv[]) {
    (void)cd;
    if (objc != 3) {
        Tcl_WrongNumArgs(interp, 1, objv, "handle key");
        return TCL_ERROR;
    }
    TricHandle *th = lookup_handle(interp, Tcl_GetString(objv[1]));
    if (th == NULL) return TCL_ERROR;
    Tcl_Size key_len;
    const char *key = Tcl_GetStringFromObj(objv[2], &key_len);
    TricValue v = read_value(&th->handle, (const uint8_t *)key, (size_t)key_len);
    if (v.data == NULL) {
        Tcl_SetObjResult(interp, Tcl_NewObj());
        return TCL_OK;
    }
    Tcl_SetObjResult(interp, Tcl_NewByteArrayObj(v.data, (Tcl_Size)v.length));
    delete_value_result(&v);
    return TCL_OK;
}

static int write_cmd(ClientData cd, Tcl_Interp *interp, int objc, Tcl_Obj *const objv[]) {
    (void)cd;
    if (objc != 4) {
        Tcl_WrongNumArgs(interp, 1, objv, "handle key value");
        return TCL_ERROR;
    }
    TricHandle *th = lookup_handle(interp, Tcl_GetString(objv[1]));
    if (th == NULL) return TCL_ERROR;
    Tcl_Size key_len, value_len;
    const char *key = Tcl_GetStringFromObj(objv[2], &key_len);
    const char *value = Tcl_GetStringFromObj(objv[3], &value_len);
    int result =
        write_value(&th->handle, (const uint8_t *)key, (size_t)key_len, (const uint8_t *)value, (size_t)value_len);
    if (result != 0) {
        Tcl_SetObjResult(interp, Tcl_NewStringObj("write failed", -1));
        return TCL_ERROR;
    }
    return TCL_OK;
}

static int del_cmd(ClientData cd, Tcl_Interp *interp, int objc, Tcl_Obj *const objv[]) {
    (void)cd;
    if (objc != 3) {
        Tcl_WrongNumArgs(interp, 1, objv, "handle key");
        return TCL_ERROR;
    }
    TricHandle *th = lookup_handle(interp, Tcl_GetString(objv[1]));
    if (th == NULL) return TCL_ERROR;
    Tcl_Size key_len;
    const char *key = Tcl_GetStringFromObj(objv[2], &key_len);
    int result = delete_value(&th->handle, (const uint8_t *)key, (size_t)key_len);
    if (result != 0) {
        Tcl_SetObjResult(interp, Tcl_NewStringObj("del failed", -1));
        return TCL_ERROR;
    }
    return TCL_OK;
}

static int cad_cmd(ClientData cd, Tcl_Interp *interp, int objc, Tcl_Obj *const objv[]) {
    (void)cd;
    if (objc != 4) {
        Tcl_WrongNumArgs(interp, 1, objv, "handle key expected");
        return TCL_ERROR;
    }
    TricHandle *th = lookup_handle(interp, Tcl_GetString(objv[1]));
    if (th == NULL) return TCL_ERROR;
    Tcl_Size key_len, expected_len;
    const char *key = Tcl_GetStringFromObj(objv[2], &key_len);
    const char *expected = Tcl_GetStringFromObj(objv[3], &expected_len);
    int result = delete_value_if_match(&th->handle,
                                       (const uint8_t *)key,
                                       (size_t)key_len,
                                       (const uint8_t *)expected,
                                       (size_t)expected_len);
    if (result < 0) {
        Tcl_SetObjResult(interp, Tcl_NewStringObj("cad failed", -1));
        return TCL_ERROR;
    }
    Tcl_SetObjResult(interp, Tcl_NewBooleanObj(result == 1));
    return TCL_OK;
}

static int ttl_cmd(ClientData cd, Tcl_Interp *interp, int objc, Tcl_Obj *const objv[]) {
    (void)cd;
    if (objc != 4) {
        Tcl_WrongNumArgs(interp, 1, objv, "handle key durationMs");
        return TCL_ERROR;
    }
    TricHandle *th = lookup_handle(interp, Tcl_GetString(objv[1]));
    if (th == NULL) return TCL_ERROR;
    Tcl_Size key_len;
    const char *key = Tcl_GetStringFromObj(objv[2], &key_len);
    Tcl_WideInt duration_ms;
    if (Tcl_GetWideIntFromObj(interp, objv[3], &duration_ms) != TCL_OK) {
        return TCL_ERROR;
    }
    int result = write_ttl(&th->handle, (const uint8_t *)key, (size_t)key_len, (uint64_t)duration_ms);
    if (result != 0) {
        Tcl_SetObjResult(interp, Tcl_NewStringObj("ttl failed", -1));
        return TCL_ERROR;
    }
    return TCL_OK;
}

static int scan_cmd(ClientData cd, Tcl_Interp *interp, int objc, Tcl_Obj *const objv[]) {
    (void)cd;
    if (objc != 3) {
        Tcl_WrongNumArgs(interp, 1, objv, "handle prefix");
        return TCL_ERROR;
    }
    TricHandle *th = lookup_handle(interp, Tcl_GetString(objv[1]));
    if (th == NULL) return TCL_ERROR;
    Tcl_Size prefix_len;
    const char *prefix = Tcl_GetStringFromObj(objv[2], &prefix_len);
    TricScanResult sr = find_by_prefix(&th->handle, (const uint8_t *)prefix, (size_t)prefix_len);
    Tcl_Obj *list = Tcl_NewListObj(0, NULL);
    for (size_t i = 0; i < sr.count; ++i) {
        Tcl_ListObjAppendElement(interp, list, Tcl_NewByteArrayObj(sr.pairs[i].key, (Tcl_Size)sr.pairs[i].key_length));
        Tcl_ListObjAppendElement(interp,
                                 list,
                                 Tcl_NewByteArrayObj(sr.pairs[i].value, (Tcl_Size)sr.pairs[i].value_length));
    }
    delete_scan_result(&sr);
    Tcl_SetObjResult(interp, list);
    return TCL_OK;
}

int Tric_Init(Tcl_Interp *interp) {
    if (Tcl_InitStubs(interp, "9.0", 0) == NULL) {
        return TCL_ERROR;
    }
    Tcl_InitHashTable(&g_handles, TCL_STRING_KEYS);
    Tcl_CreateObjCommand(interp, "tric::connect", connect_cmd, NULL, NULL);
    Tcl_CreateObjCommand(interp, "tric::disconnect", disconnect_cmd, NULL, NULL);
    Tcl_CreateObjCommand(interp, "tric::valid", valid_cmd, NULL, NULL);
    Tcl_CreateObjCommand(interp, "tric::read", read_cmd, NULL, NULL);
    Tcl_CreateObjCommand(interp, "tric::write", write_cmd, NULL, NULL);
    Tcl_CreateObjCommand(interp, "tric::del", del_cmd, NULL, NULL);
    Tcl_CreateObjCommand(interp, "tric::cad", cad_cmd, NULL, NULL);
    Tcl_CreateObjCommand(interp, "tric::ttl", ttl_cmd, NULL, NULL);
    Tcl_CreateObjCommand(interp, "tric::scan", scan_cmd, NULL, NULL);
    return Tcl_PkgProvide(interp, "tric", "0.0.0");
}
