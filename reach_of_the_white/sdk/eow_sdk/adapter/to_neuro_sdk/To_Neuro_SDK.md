# To Neuro SDK

---

## Overview

The `(from_eow_sdk_)to_neuro_sdk` component of the `Neuro Adapter` enables Neuro-sama to connect to games or applications supported by `EOW_SDK` by translating the EOW API into the Neuro API. In the following sections, this component is referred to as `E2NA`.

`E2NA` is hosted by the `Neuro Client` and uses `instance: neuro` and `port: 1219` as its default identity. It establishes a communication connection with an `EOW_SDK` instance, receives standard-compliant `SDK_Envelope` messages, and converts them into `Neuro-SDK`-compatible messages for use by the `Neuro Client`.

---

## Translation

For the EOW API → Neuro API translation specification implemented by `E2NA`, see [Neuro Adapter].

`state.snapshot` is intentionally unsupported. E2NA treats it as an invalid business payload and does not translate or forward its `data` to Neuro-SDK.

[Neuro Adapter]: ../../../../../docs/Neuro_Adapter.md

---

## Runtime

`E2NA` is an independent IPv4 TCP server. It listens on `127.0.0.1:1219` by default, performs the server side of the SDK handshake as instance `neuro`, validates framed `SDK_Envelope` JSON, and submits ordered batches of serialized `Neuro-SDK`-compatible JSON to a bounded queue owned by `Neuro Client`. It does not depend on the client-side `transport` runtime.

Different `sdk_id` values may be connected concurrently. A second active connection using the same `sdk_id` is denied during the handshake. Per-connection action state is discarded on shutdown or disconnection.

## Lifecycle

A successful handshake creates an unstarted session. The first accepted business request must be `startup`. A duplicate `startup`, or any other business request before startup, receives a failed acknowledgment. A successful `shutdown` receives an acknowledgment and then closes the connection.

Requests are acknowledged only after their complete Neuro output batch has entered the Neuro Client queue. A full queue produces a failed acknowledgment without committing action-cache changes. A disconnected queue produces a failed acknowledgment and closes the session. An output-free successful operation, such as an unchanged `action.update`, does not require a queue entry.

Frame, Envelope identity, and routing violations close the connection. A structurally valid request with an invalid business payload receives a failed acknowledgment and leaves the connection open.
