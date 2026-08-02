# Neuro Client

The Neuro Client hosts E2NA and owns its bounded output queue. The current executable starts `E2NA` with its default `127.0.0.1:1219` endpoint and consumes serialized `Neuro-SDK`-compatible JSON in batch order. The queue consumer is the integration boundary for the future upstream Neuro Client implementation.