# Transport

## TCP

`EOW_SDK` 实例与 `EOW` 实例基于本地 `TCP`，使用 `SDK_Envelope` 进行通信，默认使用 `127.0.0.1`(IPv4) 作为地址

### TCP Transportation

`TCP` 字节流切割使用以下规范：

[ `length_prefix` : `4` bytes ] [ `body` : `length_prefix` bytes ] [ `sentinel` : `5` bytes ]

- `length_prefix`

    报文长度前缀，固定 `4` 字节，无符号整数，大端序(网络字节序)，其值为紧随其后的 `body` 段的字节数

    `length_prefix` 的值不得超过 `20` MB(即 `20` * `1024` * `1024` 字节)、不得为 `0`，否则中断 `TCP` 连接

- `body`

    报文内容物，长度为 `length_prefix` 指定的字节数，可由任意非空字节序列组成

- `sentinel`

    报文尾哨兵，固定为 `5` 字节序列 `\eow/`：`0x5C` `0x65` `0x6F` `0x77` `0x2F`

    `sentinel` 不参与边界裁定，仅作帧完整性校验。接收方读毕 `length_prefix` 指定的 `body` 字节后，继续读取固定 `5` 字节，校验其是否等于 `\eow/`。若校验失败，则中断 `TCP` 连接

### TCP Connection

`EOW_SDK` 实例使用宿主传入的端口号连接本地 `EOW` 实例

成功连接后，`EOW_SDK` 实例首先向 `EOW` 实例发送握手报文

```json
{
    "handshake": {
        "schema": "1.0",
        "sdk_id": "<sdk_id>"
    }
}
```

`EOW` 实例确认 `EOW_SDK` 实例合法性，随后返回

```json
{
    "handshake": {
        "result": "accept" | "deny",
        "reason": null | "invalid_identity" | "identity_in_use" | "schema_incompatible"
    }
}
```

之后双方使用 `SDK_Envelope` 进行通信，或中断 `TCP` 连接

## SDK_Envelope

`EOW_SDK` 采用以下形式作为标准通信格式

```json
{
    "envelope_schema": "1.0",
    "envelope_kind": "request" | "response" | "event",
    "source": "<source>",
    "target": "<target>",
    "event_id": "<12digit_string>",
    "correlation_id": null | "<12digit_string>",
    "ts_utc": "<iso8601_utc>",
    "payload": { 
        "sdk_id": "<sdk_id>",
        "type": "<type>",
        ...
    }
}
```

### event_id

`event_id` 为 `12位纯数字字符串`，格式如下:

> [1-digit code] + [9-digit timestamp] + [2-digit random number]

其中，首位 `1 位代号` 始终为 `7`；随后 `9 位时间戳` 格式为 `HHMMSSMMM`，即 `时分秒毫秒`，使用 `UTC` 时间；末位 `2 位随机数` 从 `00` 到 `99` 随机抽取

### correlation_id

当 `envelope_kind` 为 `response` 时，`correlation_id` 值为该 `response` 对应 `request` 的 `event_id` 值；否则 `correlation_id` 为 `null`
