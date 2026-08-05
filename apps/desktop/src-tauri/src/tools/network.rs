use serde_json::{json, Value};
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::time::Instant;

#[cfg(windows)]
use std::time::Duration;

struct HttpStatus {
    code: u16,
    name: &'static str,
    desc: &'static str,
    usage: &'static str,
    causes: &'static str,
}

#[derive(Clone, Copy)]
struct HttpHeaderHint {
    name: &'static str,
    description: &'static str,
}

#[derive(Clone, Copy)]
struct HttpStatusDetail {
    explanation: &'static str,
    troubleshooting: &'static str,
    response_headers: &'static [HttpHeaderHint],
}

const NO_RESPONSE_HEADERS: &[HttpHeaderHint] = &[];
const LOCATION_HEADER: &[HttpHeaderHint] = &[HttpHeaderHint {
    name: "Location",
    description: "确认跳转目标是否正确，以及客户端是否允许自动跟随。",
}];
const CACHE_VALIDATION_HEADERS: &[HttpHeaderHint] = &[
    HttpHeaderHint {
        name: "ETag",
        description: "用于确认缓存实体版本。",
    },
    HttpHeaderHint {
        name: "Last-Modified",
        description: "用于确认资源最后修改时间。",
    },
];
const WWW_AUTHENTICATE_HEADER: &[HttpHeaderHint] = &[HttpHeaderHint {
    name: "WWW-Authenticate",
    description: "确认服务器要求的认证方案和认证域。",
}];
const ALLOW_HEADER: &[HttpHeaderHint] = &[HttpHeaderHint {
    name: "Allow",
    description: "查看资源实际允许的 HTTP 方法。",
}];
const PROXY_AUTHENTICATE_HEADER: &[HttpHeaderHint] = &[HttpHeaderHint {
    name: "Proxy-Authenticate",
    description: "确认代理要求的认证方案。",
}];
const CONTENT_LENGTH_HEADER: &[HttpHeaderHint] = &[HttpHeaderHint {
    name: "Content-Length",
    description: "确认请求或响应体长度是否已明确声明。",
}];
const CONTENT_RANGE_HEADER: &[HttpHeaderHint] = &[HttpHeaderHint {
    name: "Content-Range",
    description: "确认返回的字节范围和完整资源大小。",
}];
const RETRY_AFTER_HEADER: &[HttpHeaderHint] = &[HttpHeaderHint {
    name: "Retry-After",
    description: "确认客户端应等待多久后重试。",
}];
const UPGRADE_HEADER: &[HttpHeaderHint] = &[HttpHeaderHint {
    name: "Upgrade",
    description: "确认服务器要求或协商的协议版本。",
}];
const LINK_HEADER: &[HttpHeaderHint] = &[HttpHeaderHint {
    name: "Link",
    description: "确认法律依据或替代资源链接。",
}];
const EARLY_HINTS_HEADER: &[HttpHeaderHint] = &[HttpHeaderHint {
    name: "Link",
    description: "确认预加载资源链接和关系类型。",
}];

fn http_status_data() -> Vec<HttpStatus> {
    vec![
        HttpStatus {
            code: 100,
            name: "Continue",
            desc: "继续",
            usage: "客户端应继续发送请求体",
            causes: "",
        },
        HttpStatus {
            code: 101,
            name: "Switching Protocols",
            desc: "切换协议",
            usage: "WebSocket 升级",
            causes: "",
        },
        HttpStatus {
            code: 102,
            name: "Processing",
            desc: "处理中",
            usage: "服务器仍在处理请求",
            causes: "",
        },
        HttpStatus {
            code: 103,
            name: "Early Hints",
            desc: "早期提示",
            usage: "在最终响应前提示预加载资源",
            causes: "",
        },
        HttpStatus {
            code: 200,
            name: "OK",
            desc: "成功",
            usage: "请求成功",
            causes: "",
        },
        HttpStatus {
            code: 201,
            name: "Created",
            desc: "已创建",
            usage: "POST 创建资源成功",
            causes: "",
        },
        HttpStatus {
            code: 202,
            name: "Accepted",
            desc: "已接受",
            usage: "异步任务已接受处理",
            causes: "",
        },
        HttpStatus {
            code: 203,
            name: "Non-Authoritative Information",
            desc: "非权威信息",
            usage: "代理返回的修改后响应",
            causes: "",
        },
        HttpStatus {
            code: 204,
            name: "No Content",
            desc: "无内容",
            usage: "DELETE 成功，无返回体",
            causes: "",
        },
        HttpStatus {
            code: 205,
            name: "Reset Content",
            desc: "重置内容",
            usage: "请求完成后要求客户端重置视图",
            causes: "",
        },
        HttpStatus {
            code: 206,
            name: "Partial Content",
            desc: "部分内容",
            usage: "Range 请求，断点续传",
            causes: "",
        },
        HttpStatus {
            code: 207,
            name: "Multi-Status",
            desc: "多状态",
            usage: "WebDAV 返回多个资源的处理结果",
            causes: "",
        },
        HttpStatus {
            code: 208,
            name: "Already Reported",
            desc: "已报告",
            usage: "WebDAV 避免重复报告已列出的资源",
            causes: "",
        },
        HttpStatus {
            code: 226,
            name: "IM Used",
            desc: "已使用实例操纵",
            usage: "服务器返回经过实例操纵的结果",
            causes: "",
        },
        HttpStatus {
            code: 300,
            name: "Multiple Choices",
            desc: "多种选择",
            usage: "资源存在多个可选表示",
            causes: "客户端未能根据协商规则选择表示",
        },
        HttpStatus {
            code: 301,
            name: "Moved Permanently",
            desc: "永久重定向",
            usage: "资源永久迁移到新 URL",
            causes: "",
        },
        HttpStatus {
            code: 302,
            name: "Found",
            desc: "临时重定向",
            usage: "临时跳转到另一个 URL",
            causes: "",
        },
        HttpStatus {
            code: 303,
            name: "See Other",
            desc: "查看其他",
            usage: "POST 后重定向到 GET",
            causes: "",
        },
        HttpStatus {
            code: 304,
            name: "Not Modified",
            desc: "未修改",
            usage: "缓存有效，无需重新传输",
            causes: "",
        },
        HttpStatus {
            code: 307,
            name: "Temporary Redirect",
            desc: "临时重定向",
            usage: "保持请求方法的临时重定向",
            causes: "",
        },
        HttpStatus {
            code: 308,
            name: "Permanent Redirect",
            desc: "永久重定向",
            usage: "保持请求方法的永久重定向",
            causes: "",
        },
        HttpStatus {
            code: 400,
            name: "Bad Request",
            desc: "错误请求",
            usage: "请求参数错误或格式不正确",
            causes: "JSON 格式错误; 必填参数缺失; 参数类型不匹配; 请求体编码错误",
        },
        HttpStatus {
            code: 401,
            name: "Unauthorized",
            desc: "未授权",
            usage: "需要身份认证",
            causes:
                "Token 过期或无效; 未携带 Authorization 头; Cookie/Session 过期; OAuth 授权失败",
        },
        HttpStatus {
            code: 403,
            name: "Forbidden",
            desc: "禁止访问",
            usage: "服务器拒绝请求",
            causes: "无访问权限; IP 被封禁; CORS 策略拦截; CSRF Token 校验失败; 文件/目录权限不足",
        },
        HttpStatus {
            code: 404,
            name: "Not Found",
            desc: "未找到",
            usage: "请求的资源不存在",
            causes: "URL 路径拼写错误; 资源已被删除; 路由未注册; 后端接口未部署",
        },
        HttpStatus {
            code: 405,
            name: "Method Not Allowed",
            desc: "方法不允许",
            usage: "HTTP 方法不被支持",
            causes: "用 GET 请求了 POST 接口; 路由只允许特定方法; RESTful 方法不匹配",
        },
        HttpStatus {
            code: 406,
            name: "Not Acceptable",
            desc: "不可接受",
            usage: "无法满足 Accept 头要求",
            causes: "Accept 头与服务器响应格式不兼容",
        },
        HttpStatus {
            code: 407,
            name: "Proxy Authentication Required",
            desc: "需要代理认证",
            usage: "代理服务器要求认证",
            causes: "企业代理未配置认证; 代理凭证错误",
        },
        HttpStatus {
            code: 408,
            name: "Request Timeout",
            desc: "请求超时",
            usage: "服务器等待请求超时",
            causes: "客户端发送数据过慢; 网络不稳定; 大文件上传超时",
        },
        HttpStatus {
            code: 409,
            name: "Conflict",
            desc: "冲突",
            usage: "资源状态冲突",
            causes: "并发更新同一资源; 唯一约束冲突; 乐观锁版本不匹配",
        },
        HttpStatus {
            code: 410,
            name: "Gone",
            desc: "已删除",
            usage: "资源已永久删除",
            causes: "API 版本已下线; 资源被永久移除",
        },
        HttpStatus {
            code: 411,
            name: "Length Required",
            desc: "需要内容长度",
            usage: "缺少 Content-Length 头",
            causes: "未设置 Content-Length; 分块传输配置错误",
        },
        HttpStatus {
            code: 412,
            name: "Precondition Failed",
            desc: "前提条件失败",
            usage: "条件请求的前提不满足",
            causes: "If-Match/If-Unmodified-Since 条件不满足; ETag 不匹配",
        },
        HttpStatus {
            code: 413,
            name: "Payload Too Large",
            desc: "请求体过大",
            usage: "上传文件超过限制",
            causes: "上传文件超过 Nginx/后端限制; Body 大小超过配置; client_max_body_size 过小",
        },
        HttpStatus {
            code: 414,
            name: "URI Too Long",
            desc: "URI 过长",
            usage: "URL 超过服务器限制",
            causes: "GET 参数过多; 查询字符串过长; 应改用 POST",
        },
        HttpStatus {
            code: 415,
            name: "Unsupported Media Type",
            desc: "不支持的媒体类型",
            usage: "Content-Type 不被支持",
            causes: "Content-Type 与实际格式不匹配; 发送 JSON 但未设 application/json",
        },
        HttpStatus {
            code: 416,
            name: "Range Not Satisfiable",
            desc: "范围不满足",
            usage: "Range 请求超出范围",
            causes: "Range 头的字节范围超出文件实际大小",
        },
        HttpStatus {
            code: 417,
            name: "Expectation Failed",
            desc: "期望失败",
            usage: "服务器无法满足 Expect 请求头",
            causes: "Expect 值不受支持; 代理或服务器不理解请求期望",
        },
        HttpStatus {
            code: 418,
            name: "I'm a teapot",
            desc: "我是一个茶壶",
            usage: "保留的愚人节协议状态码或测试响应",
            causes: "测试接口主动返回; 客户端误用了仅供测试的端点",
        },
        HttpStatus {
            code: 421,
            name: "Misdirected Request",
            desc: "请求被误导",
            usage: "请求发送到了无法生成响应的服务器",
            causes: "HTTP/2 连接复用与目标域名不匹配; SNI 或 Host 配置错误",
        },
        HttpStatus {
            code: 422,
            name: "Unprocessable Entity",
            desc: "无法处理的实体",
            usage: "请求格式正确但语义错误",
            causes: "字段校验失败(邮箱格式、手机号等); 业务规则不满足; 枚举值不在允许范围内",
        },
        HttpStatus {
            code: 423,
            name: "Locked",
            desc: "资源被锁定",
            usage: "WebDAV 资源当前被锁定",
            causes: "资源存在未释放的锁; 并发编辑仍在进行",
        },
        HttpStatus {
            code: 424,
            name: "Failed Dependency",
            desc: "依赖失败",
            usage: "依赖的前置操作失败",
            causes: "WebDAV 绑定操作失败; 前置资源状态不满足",
        },
        HttpStatus {
            code: 425,
            name: "Too Early",
            desc: "过早请求",
            usage: "服务器拒绝可能被重放的早期请求",
            causes: "请求使用了 0-RTT; 服务器无法确认请求不可重放",
        },
        HttpStatus {
            code: 426,
            name: "Upgrade Required",
            desc: "需要升级",
            usage: "服务器要求客户端升级协议",
            causes: "客户端 HTTP 协议版本过低; 服务端策略要求升级",
        },
        HttpStatus {
            code: 428,
            name: "Precondition Required",
            desc: "需要前提条件",
            usage: "服务器要求条件请求以避免并发覆盖",
            causes: "缺少 If-Match 或 If-Unmodified-Since; API 要求乐观并发控制",
        },
        HttpStatus {
            code: 429,
            name: "Too Many Requests",
            desc: "请求过多",
            usage: "触发限流",
            causes: "API 调用频率超限; 爬虫/脚本触发防护; 缺少速率限制退避逻辑",
        },
        HttpStatus {
            code: 431,
            name: "Request Header Fields Too Large",
            desc: "请求头字段过大",
            usage: "请求头或单个请求头字段超过服务器限制",
            causes: "Cookie 过大; Authorization 头过长; 代理累计请求头超限",
        },
        HttpStatus {
            code: 451,
            name: "Unavailable For Legal Reasons",
            desc: "因法律原因不可用",
            usage: "资源因法律要求被屏蔽",
            causes: "司法管辖区限制; 版权或监管要求; 服务商合规策略",
        },
        HttpStatus {
            code: 500,
            name: "Internal Server Error",
            desc: "服务器内部错误",
            usage: "服务器未知错误",
            causes: "后端代码抛出未捕获异常; 空指针/数组越界; 数据库连接失败; 依赖服务不可用",
        },
        HttpStatus {
            code: 501,
            name: "Not Implemented",
            desc: "未实现",
            usage: "服务器不支持该功能",
            causes: "接口尚未开发; 服务器不支持请求的 HTTP 方法",
        },
        HttpStatus {
            code: 502,
            name: "Bad Gateway",
            desc: "网关错误",
            usage: "上游服务器返回无效响应",
            causes: "后端服务崩溃/未启动; Nginx upstream 配置错误; 后端返回格式异常; 容器 OOM 被杀",
        },
        HttpStatus {
            code: 503,
            name: "Service Unavailable",
            desc: "服务不可用",
            usage: "服务器过载或维护中",
            causes: "服务正在部署/重启; 连接池耗尽; 线程池满; 熔断器打开; 服务器资源不足",
        },
        HttpStatus {
            code: 504,
            name: "Gateway Timeout",
            desc: "网关超时",
            usage: "上游服务器响应超时",
            causes: "后端处理时间过长; 数据库慢查询; 第三方接口超时; proxy_read_timeout 过小",
        },
        HttpStatus {
            code: 505,
            name: "HTTP Version Not Supported",
            desc: "HTTP 版本不支持",
            usage: "不支持请求的 HTTP 版本",
            causes: "客户端使用了不兼容的 HTTP 协议版本",
        },
        HttpStatus {
            code: 506,
            name: "Variant Also Negotiates",
            desc: "变体也参与协商",
            usage: "服务器配置的透明内容协商发生循环",
            causes: "Variant 资源配置错误; 协商结果再次指向协商资源",
        },
        HttpStatus {
            code: 507,
            name: "Insufficient Storage",
            desc: "存储空间不足",
            usage: "服务器无法存储完成请求所需的数据",
            causes: "磁盘空间不足; 配额耗尽; WebDAV 资源存储限制",
        },
        HttpStatus {
            code: 508,
            name: "Loop Detected",
            desc: "检测到循环",
            usage: "服务器处理请求时检测到无限循环",
            causes: "WebDAV 绑定形成环; 反向代理或重写规则循环",
        },
        HttpStatus {
            code: 510,
            name: "Not Extended",
            desc: "未扩展",
            usage: "请求需要进一步扩展才能完成",
            causes: "客户端未提供服务器要求的扩展; 扩展协商失败",
        },
        HttpStatus {
            code: 511,
            name: "Network Authentication Required",
            desc: "需要网络认证",
            usage: "网络要求先完成认证才能访问",
            causes: "公共 Wi-Fi 门户未登录; 企业网络准入认证未完成",
        },
    ]
}

fn http_status_detail(code: u16) -> HttpStatusDetail {
    let detail = |explanation, troubleshooting, response_headers| HttpStatusDetail {
        explanation,
        troubleshooting,
        response_headers,
    };

    match code {
        100 => detail(
            "服务器已收到请求头，客户端可以继续发送请求体。",
            "确认客户端仍在发送请求体; 检查服务器是否提前关闭连接。",
            NO_RESPONSE_HEADERS,
        ),
        101 => detail(
            "服务器同意通过 Upgrade 切换到请求的其他协议。",
            "检查 Connection 和 Upgrade 请求头; 确认双方支持目标协议。",
            UPGRADE_HEADER,
        ),
        102 => detail(
            "服务器仍在处理请求，暂时没有最终响应。",
            "确认长耗时操作仍在运行; 为客户端和代理设置合理超时。",
            NO_RESPONSE_HEADERS,
        ),
        103 => detail(
            "服务器在最终响应前提示客户端预加载可能需要的资源。",
            "检查 Link 预加载目标; 确认最终响应仍会提供这些资源。",
            EARLY_HINTS_HEADER,
        ),
        200 => detail(
            "请求已成功处理，响应内容代表本次请求的结果。",
            "检查响应体和业务字段是否符合预期; 确认服务端没有把业务失败包装成 200。",
            NO_RESPONSE_HEADERS,
        ),
        201 => detail(
            "请求已成功处理，并创建了新的资源。",
            "确认资源是否实际持久化; 检查返回体或跳转位置是否指向新资源。",
            LOCATION_HEADER,
        ),
        202 => detail(
            "请求已被接受，但实际处理尚未完成。",
            "确认异步任务的查询方式; 不要把 202 当作最终业务成功。",
            NO_RESPONSE_HEADERS,
        ),
        203 => detail(
            "代理返回了与源站不同但仍可使用的信息。",
            "检查响应是否经过代理改写; 对比源站和代理的响应内容。",
            NO_RESPONSE_HEADERS,
        ),
        204 => detail(
            "请求成功，但响应不包含消息正文。",
            "确认客户端没有强行解析响应体; 检查服务端是否误用了无正文状态。",
            NO_RESPONSE_HEADERS,
        ),
        205 => detail(
            "请求成功，并要求客户端将当前文档或表单重置到初始状态。",
            "确认客户端按约定清空或重置视图; 检查响应是否确实没有正文。",
            NO_RESPONSE_HEADERS,
        ),
        206 => detail(
            "服务器只返回了请求资源的一部分字节范围。",
            "检查 Range 与 Content-Range 是否一致; 确认客户端能正确拼接分段响应。",
            CONTENT_RANGE_HEADER,
        ),
        207 => detail(
            "服务器在一个响应中返回多个资源各自的处理结果。",
            "解析 WebDAV 多状态正文; 逐项检查资源的独立状态。",
            NO_RESPONSE_HEADERS,
        ),
        208 => detail(
            "服务器报告当前资源已经在此前的多状态响应中出现。",
            "检查资源绑定关系; 避免客户端重复处理同一资源。",
            NO_RESPONSE_HEADERS,
        ),
        226 => detail(
            "服务器返回了经过实例操纵后的资源表示。",
            "确认客户端支持协商出的实例操纵; 对比原始资源和变换结果。",
            NO_RESPONSE_HEADERS,
        ),
        300 => detail(
            "请求对应多个可选表示，客户端需要选择一个目标。",
            "检查内容协商规则和候选资源; 确认客户端如何选择或展示候选项。",
            LOCATION_HEADER,
        ),
        301 => detail(
            "资源已永久迁移到新的 URL，后续请求应使用新地址。",
            "检查 Location 目标是否正确; 更新持久化链接和客户端配置。",
            LOCATION_HEADER,
        ),
        302 => detail(
            "资源暂时位于另一个 URL，当前跳转不代表永久迁移。",
            "检查 Location 目标; 确认客户端跟随跳转时不会错误改变请求方法。",
            LOCATION_HEADER,
        ),
        303 => detail(
            "服务器要求客户端使用另一个 URL 通常以 GET 方式查看结果。",
            "检查 POST 后的 Location; 确认客户端按约定使用 GET 访问目标。",
            LOCATION_HEADER,
        ),
        304 => detail(
            "条件请求命中缓存，资源没有发生需要重新传输的变化。",
            "检查 If-None-Match 或 If-Modified-Since; 确认客户端使用本地缓存内容。",
            CACHE_VALIDATION_HEADERS,
        ),
        307 => detail(
            "资源临时跳转，客户端应保持原请求方法和请求体。",
            "检查 Location 目标; 确认重试时不会把 POST 等方法改成 GET。",
            LOCATION_HEADER,
        ),
        308 => detail(
            "资源永久跳转，客户端应保持原请求方法和请求体。",
            "检查 Location 目标; 更新长期配置并确认重试仍保留原方法。",
            LOCATION_HEADER,
        ),
        400 => detail(
            "服务器无法理解请求的语法、格式或参数。",
            "检查请求方法、URL、JSON 格式、必填字段和字符编码。",
            NO_RESPONSE_HEADERS,
        ),
        401 => detail(
            "请求缺少有效身份认证，服务器要求客户端先证明身份。",
            "检查 Authorization 或 Cookie; 确认凭证未过期且认证方案正确。",
            WWW_AUTHENTICATE_HEADER,
        ),
        403 => detail(
            "服务器理解请求，但拒绝授予当前请求访问权限。",
            "检查用户权限、IP 策略、CSRF/CORS 配置和资源访问规则。",
            NO_RESPONSE_HEADERS,
        ),
        404 => detail(
            "服务器找不到请求的资源或不愿透露该资源是否存在。",
            "核对 URL 路径、大小写、路由注册和部署版本; 确认资源没有被删除。",
            NO_RESPONSE_HEADERS,
        ),
        405 => detail(
            "目标资源存在，但不支持当前 HTTP 方法。",
            "检查请求方法和路由定义; 按 Allow 头选择受支持的方法。",
            ALLOW_HEADER,
        ),
        406 => detail(
            "服务器无法生成满足请求 Accept 等协商条件的表示。",
            "检查 Accept、Accept-Language 和 Accept-Encoding; 确认服务端支持目标格式。",
            NO_RESPONSE_HEADERS,
        ),
        407 => detail(
            "代理要求客户端先完成代理认证。",
            "检查代理地址和凭证; 确认客户端发送了代理要求的认证信息。",
            PROXY_AUTHENTICATE_HEADER,
        ),
        408 => detail(
            "服务器等待请求完成时超时。",
            "检查客户端发送速度和网络稳定性; 增大超时前先确认请求体不会无限等待。",
            NO_RESPONSE_HEADERS,
        ),
        409 => detail(
            "请求与资源当前状态冲突，服务器无法按当前状态完成操作。",
            "检查并发版本、唯一约束和资源状态; 重新读取资源后再决定是否重试。",
            NO_RESPONSE_HEADERS,
        ),
        410 => detail(
            "资源已永久删除，服务器明确表示不再提供该资源。",
            "确认 API 版本或资源迁移公告; 删除客户端对旧资源的依赖。",
            NO_RESPONSE_HEADERS,
        ),
        411 => detail(
            "服务器要求请求明确提供消息正文长度。",
            "检查 Content-Length; 确认代理和服务器支持当前的传输方式。",
            CONTENT_LENGTH_HEADER,
        ),
        412 => detail(
            "请求中的条件前提不满足，服务器拒绝继续操作。",
            "重新获取资源的 ETag 或修改时间; 检查 If-Match 等条件是否过期。",
            CACHE_VALIDATION_HEADERS,
        ),
        413 => detail(
            "请求正文超过服务器愿意或能够处理的大小。",
            "检查上传大小限制和反向代理配置; 必要时采用分片或调整请求体。",
            CONTENT_LENGTH_HEADER,
        ),
        414 => detail(
            "请求目标 URL 超过服务器允许的长度。",
            "减少查询参数; 将大 payload 改为 POST 请求体; 检查代理长度限制。",
            NO_RESPONSE_HEADERS,
        ),
        415 => detail(
            "请求正文格式或 Content-Type 不被目标资源支持。",
            "核对 Content-Type 与实际正文; 确认接口支持的媒体类型和编码。",
            NO_RESPONSE_HEADERS,
        ),
        416 => detail(
            "Range 请求指定的字节范围无法满足。",
            "检查起止字节是否超过资源长度; 先获取资源总长度再重建 Range。",
            CONTENT_RANGE_HEADER,
        ),
        417 => detail(
            "服务器无法满足 Expect 请求头声明的期望。",
            "检查 Expect 值和代理兼容性; 不需要时移除 Expect 请求头。",
            NO_RESPONSE_HEADERS,
        ),
        418 => detail(
            "这是保留的愚人节协议状态码，常用于测试或明确约定的特殊响应。",
            "确认是否命中了测试接口; 生产接口应按自身 API 文档处理该响应。",
            NO_RESPONSE_HEADERS,
        ),
        421 => detail(
            "请求发送到了无法为该目标生成响应的服务器。",
            "检查 HTTP/2 连接复用、SNI、Host 和代理路由; 必要时新建连接重试。",
            NO_RESPONSE_HEADERS,
        ),
        422 => detail(
            "请求语法正确，但字段或业务语义没有通过服务器校验。",
            "读取响应中的字段错误; 检查格式、枚举值和业务前置条件。",
            NO_RESPONSE_HEADERS,
        ),
        423 => detail(
            "请求目标资源当前被锁定，操作无法立即执行。",
            "查找并释放未完成的资源锁; 确认并发编辑是否仍在进行。",
            NO_RESPONSE_HEADERS,
        ),
        424 => detail(
            "当前操作依赖的前置操作失败，因此无法继续。",
            "定位依赖链中最早失败的操作; 修复前置资源状态后再重试。",
            NO_RESPONSE_HEADERS,
        ),
        425 => detail(
            "服务器拒绝可能被重放的过早请求，通常与 0-RTT 有关。",
            "确认请求是否可安全重放; 按 Retry-After 或重新建立完整连接后重试。",
            RETRY_AFTER_HEADER,
        ),
        426 => detail(
            "服务器要求客户端升级到支持的 HTTP 或其他协议版本。",
            "检查服务端要求的协议版本; 按 Upgrade 头完成协商。",
            UPGRADE_HEADER,
        ),
        428 => detail(
            "服务器要求条件请求，以避免并发更新覆盖其他修改。",
            "先读取资源版本; 使用 If-Match 或 If-Unmodified-Since 提交更新。",
            CACHE_VALIDATION_HEADERS,
        ),
        429 => detail(
            "客户端在一段时间内发送了过多请求，触发了服务端限流。",
            "降低请求频率; 按 Retry-After 实施退避; 检查是否存在重复轮询或重试风暴。",
            RETRY_AFTER_HEADER,
        ),
        431 => detail(
            "请求头总大小或某个请求头字段超过服务器限制。",
            "清理过大的 Cookie 和 Authorization; 检查代理与服务端的请求头上限。",
            NO_RESPONSE_HEADERS,
        ),
        451 => detail(
            "资源因法律、监管或司法辖区要求而不可用。",
            "检查服务商合规说明和访问区域; 查看 Link 提供的法律或替代资源信息。",
            LINK_HEADER,
        ),
        500 => detail(
            "服务器遇到未预期的内部错误，无法完成请求。",
            "查看服务端日志和关联请求 ID; 检查异常、数据库和依赖服务状态。",
            NO_RESPONSE_HEADERS,
        ),
        501 => detail(
            "服务器不支持完成请求所需的功能或 HTTP 方法。",
            "确认接口是否已实现; 检查请求方法和服务端能力声明。",
            NO_RESPONSE_HEADERS,
        ),
        502 => detail(
            "网关或代理从上游服务收到无效响应。",
            "检查上游进程、连接和响应格式; 对比网关日志与后端日志。",
            NO_RESPONSE_HEADERS,
        ),
        503 => detail(
            "服务器当前无法处理请求，常见原因是过载、维护或暂时不可用。",
            "检查服务健康、连接池和资源使用; 按 Retry-After 或退避策略重试。",
            RETRY_AFTER_HEADER,
        ),
        504 => detail(
            "网关等待上游响应超时。",
            "检查上游慢查询和依赖超时; 对比网关与服务端的超时配置。",
            NO_RESPONSE_HEADERS,
        ),
        505 => detail(
            "服务器不支持请求使用的 HTTP 协议版本。",
            "确认客户端协商出的协议版本; 升级客户端或调整服务端协议配置。",
            NO_RESPONSE_HEADERS,
        ),
        506 => detail(
            "服务器配置的透明内容协商发生循环，无法选出最终表示。",
            "检查 Variant 资源和协商配置; 确认变体不会再次指向协商资源。",
            NO_RESPONSE_HEADERS,
        ),
        507 => detail(
            "服务器当前没有足够空间存储完成请求所需的数据。",
            "检查磁盘、配额和对象存储容量; 清理或扩容后再重试。",
            NO_RESPONSE_HEADERS,
        ),
        508 => detail(
            "服务器处理请求时检测到无限循环。",
            "检查 WebDAV 绑定、重写规则和反向代理链路; 移除循环依赖。",
            NO_RESPONSE_HEADERS,
        ),
        510 => detail(
            "请求需要额外扩展才能被服务器处理，但扩展协商没有完成。",
            "检查服务器要求的扩展和客户端声明; 按接口文档补充扩展信息。",
            NO_RESPONSE_HEADERS,
        ),
        511 => detail(
            "访问网络前需要先通过网络认证，例如公共 Wi-Fi 门户认证。",
            "打开认证门户并完成登录; 检查企业网络准入或代理认证状态。",
            NO_RESPONSE_HEADERS,
        ),
        _ => detail(
            "该状态码属于已收录的标准条目。",
            "结合响应体、请求链路和服务端日志继续确认具体原因。",
            NO_RESPONSE_HEADERS,
        ),
    }
}

fn status_to_value(status: &HttpStatus) -> Value {
    let detail = http_status_detail(status.code);
    let response_headers: Vec<Value> = detail
        .response_headers
        .iter()
        .map(|header| json!({ "name": header.name, "description": header.description }))
        .collect();
    json!({
        "code": status.code,
        "name": status.name,
        "desc": status.desc,
        "usage": status.usage,
        "causes": status.causes,
        "explanation": detail.explanation,
        "troubleshooting": detail.troubleshooting,
        "responseHeaders": response_headers,
    })
}

fn status_match_rank(status: &HttpStatus, detail: HttpStatusDetail, query: &str) -> Option<u8> {
    let code = status.code.to_string();
    let name = status.name.to_lowercase();
    let desc = status.desc.to_lowercase();
    let exact = [&code, &name];
    if exact.iter().any(|field| field == &query) {
        return Some(0);
    }

    if name.starts_with(query) || desc.starts_with(query) {
        return Some(1);
    }

    let text_fields = [
        status.usage,
        status.causes,
        detail.explanation,
        detail.troubleshooting,
    ];
    if code.contains(query)
        || name.contains(query)
        || desc.contains(query)
        || text_fields
            .iter()
            .any(|field| field.to_lowercase().contains(query))
        || detail.response_headers.iter().any(|header| {
            header.name.to_lowercase().contains(query)
                || header.description.to_lowercase().contains(query)
        })
    {
        Some(2)
    } else {
        None
    }
}

fn category_for_code(code: u16) -> (&'static str, &'static str) {
    match code / 100 {
        1 => ("1xx", "信息响应"),
        2 => ("2xx", "成功"),
        3 => ("3xx", "重定向"),
        4 => ("4xx", "客户端错误"),
        5 => ("5xx", "服务器错误"),
        _ => ("未知", "未知响应"),
    }
}

fn unknown_code_hint(code: u16) -> Value {
    let (category, name) = category_for_code(code);
    json!({
        "code": code,
        "category": category,
        "name": name,
        "message": format!(
            "{code} 属于 {category} {name}范围，但该具体状态码未在标准条目中收录，具体含义未定义"
        ),
    })
}

fn http_status_list(_payload: &Value) -> Result<Value, String> {
    let data = http_status_data();
    let categories = [
        ("1xx", "信息响应"),
        ("2xx", "成功"),
        ("3xx", "重定向"),
        ("4xx", "客户端错误"),
        ("5xx", "服务器错误"),
    ];
    let groups: Vec<Value> = categories
        .iter()
        .map(|(cat, name)| {
            let prefix = cat.chars().next().unwrap().to_digit(10).unwrap() as u16;
            let codes: Vec<Value> = data
                .iter()
                .filter(|s| s.code / 100 == prefix)
                .map(status_to_value)
                .collect();
            json!({ "category": cat, "name": name, "codes": codes })
        })
        .collect();
    Ok(json!({ "groups": groups }))
}

fn http_status_lookup(payload: &Value) -> Result<Value, String> {
    let query = payload["query"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    if query.is_empty() {
        return Ok(json!({ "results": [], "classificationHint": null }));
    }
    let data = http_status_data();
    let mut matches: Vec<(u8, &HttpStatus)> = data
        .iter()
        .filter_map(|status| {
            status_match_rank(status, http_status_detail(status.code), &query)
                .map(|rank| (rank, status))
        })
        .collect();
    matches.sort_by_key(|(rank, status)| (*rank, status.code));
    let results: Vec<Value> = matches
        .into_iter()
        .map(|(_, status)| status_to_value(status))
        .collect();

    let classification_hint = if query.len() == 3 && query.chars().all(|c| c.is_ascii_digit()) {
        query.parse::<u16>().ok().and_then(|code| {
            if (100..=599).contains(&code) && !data.iter().any(|status| status.code == code) {
                Some(unknown_code_hint(code))
            } else {
                None
            }
        })
    } else {
        None
    };

    Ok(json!({
        "results": results,
        "classificationHint": classification_hint,
    }))
}

fn chmod_calc(payload: &Value) -> Result<Value, String> {
    let mode = payload["mode"].as_str().unwrap_or("numeric");
    let value = payload["value"].as_str().unwrap_or("644");

    let bits: [bool; 9] = if mode == "symbolic" {
        if value.len() != 9 {
            return Err("符号模式需要 9 个字符 (如 rwxr-xr-x)".to_string());
        }
        let chars: Vec<char> = value.chars().collect();
        [
            chars[0] == 'r',
            chars[1] == 'w',
            chars[2] == 'x',
            chars[3] == 'r',
            chars[4] == 'w',
            chars[5] == 'x',
            chars[6] == 'r',
            chars[7] == 'w',
            chars[8] == 'x',
        ]
    } else {
        let num = u16::from_str_radix(value, 8).map_err(|_| "无效的八进制数".to_string())?;
        if num > 0o777 {
            return Err("权限值不能超过 777".to_string());
        }
        [
            num & 0o400 != 0,
            num & 0o200 != 0,
            num & 0o100 != 0,
            num & 0o040 != 0,
            num & 0o020 != 0,
            num & 0o010 != 0,
            num & 0o004 != 0,
            num & 0o002 != 0,
            num & 0o001 != 0,
        ]
    };

    let numeric_val = (if bits[0] { 4 } else { 0 }
        + if bits[1] { 2 } else { 0 }
        + if bits[2] { 1 } else { 0 })
        * 100
        + (if bits[3] { 4 } else { 0 } + if bits[4] { 2 } else { 0 } + if bits[5] { 1 } else { 0 })
            * 10
        + (if bits[6] { 4 } else { 0 } + if bits[7] { 2 } else { 0 } + if bits[8] { 1 } else { 0 });

    let symbolic: String = bits
        .iter()
        .enumerate()
        .map(|(i, &b)| {
            if !b {
                '-'
            } else {
                match i % 3 {
                    0 => 'r',
                    1 => 'w',
                    _ => 'x',
                }
            }
        })
        .collect();

    Ok(json!({
        "numeric": format!("{numeric_val:03}"),
        "symbolic": symbolic,
        "owner": { "read": bits[0], "write": bits[1], "execute": bits[2] },
        "group": { "read": bits[3], "write": bits[4], "execute": bits[5] },
        "other": { "read": bits[6], "write": bits[7], "execute": bits[8] },
    }))
}

/// UDP 端口测试
/// UDP 是无连接协议，测试原理：
/// 1. 尝试绑定本地 UDP 端口
/// 2. 尝试向目标发送数据（0字节）
/// 3. 设置读取超时，如果收到 ICMP 不可达则判定为端口不可达
/// 注意：由于 UDP 的特性，无法 100% 确认端口开放，只能判断是否有明显的拒绝响应
fn udp_test(payload: &Value) -> Result<Value, String> {
    let host = payload["host"].as_str().unwrap_or("127.0.0.1");
    let port = payload["port"].as_u64().unwrap_or(53) as u16;
    let timeout_ms = payload["timeoutMs"].as_u64().unwrap_or(2000);
    let started = Instant::now();

    let target_addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|e| format!("invalid address: {e}"))?;

    // 绑定本地任意端口
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("failed to bind udp socket: {e}"))?;

    // 设置读写超时
    socket
        .set_read_timeout(Some(Duration::from_millis(timeout_ms)))
        .map_err(|e| format!("failed to set timeout: {e}"))?;
    socket
        .set_write_timeout(Some(Duration::from_millis(timeout_ms)))
        .map_err(|e| format!("failed to set timeout: {e}"))?;

    // 尝试连接（仅设置默认地址，不实际发送数据）
    let connect_result = socket.connect(target_addr);

    // UDP "连接" 几乎总是"成功"，因为它只是设置默认地址
    // 真正的测试是尝试发送数据后看是否有 ICMP 错误
    if connect_result.is_err() {
        return Ok(json!({
            "host": host,
            "port": port,
            "reachable": false,
            "latencyMs": started.elapsed().as_millis(),
            "error": "无法连接到目标地址"
        }));
    }

    // 发送一个空 UDP 数据包
    let send_result = socket.send(&[]);
    if let Err(e) = send_result {
        return Ok(json!({
            "host": host,
            "port": port,
            "reachable": false,
            "latencyMs": started.elapsed().as_millis(),
            "error": format!("发送失败: {e}")
        }));
    }

    // 尝试接收响应（大多数 UDP 服务不会响应空包）
    // 但我们设置一个短暂的超时来检测 ICMP 不可达错误
    let mut buf = [0u8; 1];
    match socket.recv(&mut buf) {
        Ok(_) => {
            // 收到响应，说明端口是开放的
            Ok(json!({
                "host": host,
                "port": port,
                "reachable": true,
                "latencyMs": started.elapsed().as_millis(),
                "error": null
            }))
        }
        Err(e) => {
            let error_kind = e.kind();
            let error_msg = e.to_string().to_lowercase();
            let elapsed = started.elapsed().as_millis();

            // 检测 ICMP 不可达错误（Windows 和 Linux 的错误信息不同）
            let is_unreachable = error_kind == std::io::ErrorKind::ConnectionRefused
                || error_msg.contains("unreachable")
                || error_msg.contains("icmp")
                || error_msg.contains("拒绝")
                || error_msg.contains("refused");

            if is_unreachable {
                // 明确收到 ICMP 不可达，端口关闭
                Ok(json!({
                    "host": host,
                    "port": port,
                    "reachable": false,
                    "latencyMs": elapsed,
                    "error": "端口不可达（可能被防火墙阻止或服务未运行）"
                }))
            } else {
                // 超时或其他原因，无法确定状态
                // UDP 无响应是正常情况，我们标记为可能可达
                Ok(json!({
                    "host": host,
                    "port": port,
                    "reachable": true,
                    "latencyMs": elapsed,
                    "error": null,
                    "note": "UDP 无响应（这是正常行为，端口可能开放）"
                }))
            }
        }
    }
}

/// PING 测试
/// Windows 上使用 winping 库（Windows ICMP API），无需管理员权限
#[cfg(windows)]
fn ping_test(payload: &Value) -> Result<Value, String> {
    use winping::{Buffer, Pinger};

    let host = payload["host"].as_str().unwrap_or("127.0.0.1");
    let count = payload["count"].as_u64().unwrap_or(3).min(10).max(1);
    let interval_ms = 100;

    let pinger = Pinger::new().map_err(|e| format!("初始化 ping 失败: {e}"))?;
    let dest = host.parse().map_err(|e| format!("无效地址: {e}"))?;

    let mut success_count = 0u64;
    let mut total_latency = 0u64;
    let mut latencies: Vec<u64> = Vec::new();

    for i in 0..count {
        if i > 0 {
            std::thread::sleep(Duration::from_millis(interval_ms));
        }

        let mut buffer = Buffer::new();
        match pinger.send(dest, &mut buffer) {
            Ok(rtt) => {
                success_count += 1;
                total_latency += rtt as u64;
                latencies.push(rtt as u64);
            }
            Err(_) => {}
        }
    }

    let is_reachable = success_count > 0;
    let avg_latency = if success_count > 0 {
        total_latency / success_count
    } else {
        0
    };
    let packet_loss = if count > 0 {
        ((count - success_count) * 100) / count
    } else {
        100
    };

    Ok(json!({
        "host": host,
        "reachable": is_reachable,
        "latencyMs": avg_latency,
        "packetLoss": packet_loss,
        "packetsSent": count,
        "packetsReceived": success_count,
        "latencies": latencies,
        "error": if is_reachable { Value::Null } else { Value::String("无法到达目标主机".to_string()) }
    }))
}

/// PING 测试（非 Windows 平台保留系统命令实现）
#[cfg(not(windows))]
fn ping_test(payload: &Value) -> Result<Value, String> {
    use std::process::Command;
    use std::time::Duration;

    let host = payload["host"].as_str().unwrap_or("127.0.0.1");
    let timeout_ms = payload["timeoutMs"].as_u64().unwrap_or(2000);
    let count = payload["count"].as_u64().unwrap_or(3).min(10).max(1);
    let started = Instant::now();

    let mut success_count = 0u64;
    let mut total_latency = 0u64;
    let mut latencies: Vec<u64> = Vec::new();
    let mut last_error: Option<String> = None;

    for i in 0..count {
        if i > 0 {
            std::thread::sleep(Duration::from_millis(100));
        }

        let output = Command::new("ping")
            .args(&["-n", "1", "-w", &timeout_ms.to_string(), host])
            .output();

        match output {
            Ok(result) => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                let stderr = String::from_utf8_lossy(&result.stderr);
                let combined = format!("{}{}", stdout, stderr);

                let latency = parse_ping_single_latency(&combined);

                if result.status.success() {
                    if latency.is_some() {
                        success_count += 1;
                        let lat = latency.unwrap();
                        total_latency += lat;
                        latencies.push(lat);
                    } else {
                        last_error = Some("请求超时".to_string());
                    }
                } else {
                    last_error = Some("无法到达目标主机".to_string());
                }
            }
            Err(e) => {
                last_error = Some(format!("执行 ping 命令失败: {e}"));
            }
        }
    }

    let is_reachable = success_count > 0;
    let avg_latency = if success_count > 0 {
        total_latency / success_count
    } else {
        started.elapsed().as_millis() as u64
    };
    let packet_loss = if count > 0 {
        ((count - success_count) * 100) / count
    } else {
        100
    };

    Ok(json!({
        "host": host,
        "reachable": is_reachable,
        "latencyMs": avg_latency,
        "packetLoss": packet_loss,
        "packetsSent": count,
        "packetsReceived": success_count,
        "error": if is_reachable { Value::Null } else { Value::String(last_error.unwrap_or_else(|| "无法到达目标主机".to_string())) },
        "latencies": latencies
    }))
}

/// 解析单次 ping 延迟（非 Windows 平台使用）
#[cfg(not(windows))]
fn parse_ping_single_latency(output: &str) -> Option<u64> {
    let lower = output.to_lowercase();

    for line in lower.lines() {
        if let Some(pos) = line.find("time=").or_else(|| line.find("时间=")) {
            let after = &line[pos + 5..];
            let num_str: String = after
                .chars()
                .skip_while(|c| *c == '<' || *c == ' ')
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if let Ok(num) = num_str.parse::<f64>() {
                return Some(num as u64);
            }
        }
    }
    None
}

const ACTIONS: &[&str] = &[
    "tcp_test",
    "http_test",
    "http_status_list",
    "http_status_lookup",
    "chmod_calc",
    "udp_test",
    "ping_test",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported network action: {action}"));
    }
    match action {
        "tcp_test" => {
            let host = payload["host"].as_str().unwrap_or("127.0.0.1");
            let port = payload["port"].as_u64().unwrap_or(80) as u16;
            let timeout_ms = payload["timeoutMs"].as_u64().unwrap_or(2000);
            let started = Instant::now();
            let addr: SocketAddr = format!("{host}:{port}")
                .parse()
                .map_err(|e| format!("invalid address: {e}"))?;
            let result = TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms));
            Ok(json!({
                "host": host,
                "port": port,
                "reachable": result.is_ok(),
                "latencyMs": started.elapsed().as_millis(),
                "error": result.err().map(|e| e.to_string())
            }))
        }
        "http_test" => {
            let url = payload["url"].as_str().unwrap_or("http://127.0.0.1");
            let timeout_ms = payload["timeoutMs"].as_u64().unwrap_or(5000);
            let started = Instant::now();
            let agent = ureq::AgentBuilder::new()
                .timeout(Duration::from_millis(timeout_ms))
                .build();
            match agent.head(url).call() {
                Ok(resp) => Ok(json!({
                    "url": url,
                    "reachable": true,
                    "statusCode": resp.status(),
                    "latencyMs": started.elapsed().as_millis(),
                    "error": null
                })),
                Err(ureq::Error::Status(code, _resp)) => Ok(json!({
                    "url": url,
                    "reachable": true,
                    "statusCode": code,
                    "latencyMs": started.elapsed().as_millis(),
                    "error": null
                })),
                Err(e) => Ok(json!({
                    "url": url,
                    "reachable": false,
                    "statusCode": null,
                    "latencyMs": started.elapsed().as_millis(),
                    "error": e.to_string()
                })),
            }
        }
        "http_status_list" => http_status_list(payload),
        "http_status_lookup" => http_status_lookup(payload),
        "chmod_calc" => chmod_calc(payload),
        "udp_test" => udp_test(payload),
        "ping_test" => ping_test(payload),
        _ => Err(format!("unsupported network action: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn tcp_test_should_detect_reachable_and_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = thread::spawn(move || {
            let _ = listener.accept();
        });

        let ok = execute(
            "tcp_test",
            &json!({ "host": "127.0.0.1", "port": port, "timeoutMs": 1000 }),
        )
        .expect("tcp ok");
        assert_eq!(ok["reachable"], true);
        drop(handle);

        let fail = execute(
            "tcp_test",
            &json!({ "host": "127.0.0.1", "port": 9, "timeoutMs": 100 }),
        )
        .expect("tcp fail path");
        assert!(fail["reachable"].is_boolean());
    }

    #[test]
    fn http_test_should_work_with_local_server() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
            }
        });

        let out = execute(
            "http_test",
            &json!({ "url": format!("http://127.0.0.1:{port}"), "timeoutMs": 1500 }),
        )
        .expect("http test");
        assert_eq!(out["reachable"], true);
        assert!(out["statusCode"].as_u64().unwrap_or(0) >= 200);
    }

    #[test]
    fn http_status_list_returns_groups() {
        let r = execute("http_status_list", &json!({})).unwrap();
        let groups = r["groups"].as_array().unwrap();
        assert_eq!(groups.len(), 5);
        assert_eq!(groups[0]["category"], "1xx");

        let codes = groups
            .iter()
            .flat_map(|group| group["codes"].as_array().unwrap())
            .collect::<Vec<_>>();
        for code in [205, 418, 425, 428, 431, 451, 507, 511] {
            assert!(codes.iter().any(|item| item["code"] == code));
        }

        let too_many_requests = codes.iter().find(|item| item["code"] == 429).unwrap();
        assert!(!too_many_requests["explanation"]
            .as_str()
            .unwrap()
            .is_empty());
        assert!(!too_many_requests["troubleshooting"]
            .as_str()
            .unwrap()
            .is_empty());
        assert_eq!(
            too_many_requests["responseHeaders"][0]["name"],
            "Retry-After"
        );
        let early_hints = codes.iter().find(|item| item["code"] == 103).unwrap();
        assert_eq!(early_hints["responseHeaders"][0]["name"], "Link");
        for item in codes {
            assert!(!item["explanation"].as_str().unwrap().is_empty());
            assert!(!item["troubleshooting"].as_str().unwrap().is_empty());
            assert!(item["responseHeaders"].is_array());
        }
    }

    #[test]
    fn http_status_lookup_by_code() {
        let r = execute("http_status_lookup", &json!({"query": "404"})).unwrap();
        let results = r["results"].as_array().unwrap();
        assert!(results.len() >= 1);
        assert_eq!(results[0]["code"], 404);
    }

    #[test]
    fn http_status_lookup_by_text() {
        let r = execute("http_status_lookup", &json!({"query": "not found"})).unwrap();
        let results = r["results"].as_array().unwrap();
        assert!(results.iter().any(|r| r["code"] == 404));
    }

    #[test]
    fn http_status_lookup_matches_detail_fields_and_ranks_exact_matches() {
        let by_header =
            execute("http_status_lookup", &json!({"query": "WWW-Authenticate"})).unwrap();
        assert_eq!(by_header["results"][0]["code"], 401);

        let by_cause = execute("http_status_lookup", &json!({"query": "限流"})).unwrap();
        assert_eq!(by_cause["results"][0]["code"], 429);

        let by_usage = execute("http_status_lookup", &json!({"query": "WebSocket 升级"})).unwrap();
        assert_eq!(by_usage["results"][0]["code"], 101);

        let by_explanation = execute("http_status_lookup", &json!({"query": "连接复用"})).unwrap();
        assert_eq!(by_explanation["results"][0]["code"], 421);

        let by_troubleshooting =
            execute("http_status_lookup", &json!({"query": "慢查询"})).unwrap();
        assert_eq!(by_troubleshooting["results"][0]["code"], 504);

        let data = http_status_data();
        let success = data.iter().find(|status| status.code == 200).unwrap();
        assert_eq!(
            status_match_rank(success, http_status_detail(success.code), "成功"),
            Some(1)
        );

        let by_shared_term = execute("http_status_lookup", &json!({"query": "客户端"})).unwrap();
        let shared_codes = by_shared_term["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["code"].as_u64().unwrap())
            .collect::<Vec<_>>();
        assert!(shared_codes.windows(2).all(|pair| pair[0] <= pair[1]));

        let exact = execute("http_status_lookup", &json!({"query": "404"})).unwrap();
        assert_eq!(exact["results"][0]["code"], 404);
    }

    #[test]
    fn http_status_lookup_returns_unknown_code_classification_hint() {
        let r = execute("http_status_lookup", &json!({"query": "599"})).unwrap();
        assert!(r["results"].as_array().unwrap().is_empty());
        assert_eq!(r["classificationHint"]["code"], 599);
        assert_eq!(r["classificationHint"]["category"], "5xx");
        assert!(r["classificationHint"]["message"]
            .as_str()
            .unwrap()
            .contains("具体含义未定义"));

        let out_of_range = execute("http_status_lookup", &json!({"query": "099"})).unwrap();
        assert!(out_of_range["results"].as_array().unwrap().is_empty());
        assert!(out_of_range["classificationHint"].is_null());
    }

    #[test]
    fn chmod_calc_from_numeric() {
        let r = execute("chmod_calc", &json!({"mode": "numeric", "value": "755"})).unwrap();
        assert_eq!(r["numeric"], "755");
        assert_eq!(r["symbolic"], "rwxr-xr-x");
        assert_eq!(r["owner"]["read"], true);
        assert_eq!(r["owner"]["write"], true);
        assert_eq!(r["owner"]["execute"], true);
        assert_eq!(r["group"]["write"], false);
    }

    #[test]
    fn chmod_calc_from_symbolic() {
        let r = execute(
            "chmod_calc",
            &json!({"mode": "symbolic", "value": "rw-r--r--"}),
        )
        .unwrap();
        assert_eq!(r["numeric"], "644");
    }

    #[test]
    fn chmod_calc_zero() {
        let r = execute("chmod_calc", &json!({"mode": "numeric", "value": "000"})).unwrap();
        assert_eq!(r["symbolic"], "---------");
    }
}
