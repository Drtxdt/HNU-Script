#Requires AutoHotkey v2.0  ; 强制要求 v2.0+ 环境

; 热键：Ctrl+WIN+V 触发（可自定义，避免与系统热键冲突）
^#v:: {
    try {
        ; 激活目标窗口（替换为你的窗口标题）
        targetWindow := ""
        winObj := WinActivate(targetWindow)
        WinWaitActive(targetWindow,, 1)  ; 等待1秒，超时则报错
    } catch Error as e {
        MsgBox("无法激活窗口: " e.Message)
        return
    }

    ; 安全获取剪贴板内容
    clipboardText := A_Clipboard
    if (clipboardText = "") {
        MsgBox("剪贴板为空！")
        return
    }

    ; 设置输入延迟（单位：毫秒，根据软件响应调整）
    SetKeyDelay(30, 10)  ; 按键延迟30ms，按下/抬起间隔10ms

    ; 使用 SendText 高效发送原始文本（自动处理Unicode）
    SendText(clipboardText)

    ; 可选：针对不支持SendText的软件，逐字符发送（兼容模式）
    /*
    for chr in StrSplit(clipboardText) {
        if (Ord(chr) > 0x7F) {  ; 非ASCII字符（如数学符号）
            Send("{U+" Format("{:04X}", Ord(chr)) "}")  ; Unicode十六进制发送
        } else {
            SendText(chr)  ; ASCII字符直接发送
        }
    }
    */
}