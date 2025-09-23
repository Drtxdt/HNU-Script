#Requires AutoHotkey v2.0

; 显示启动提示（仅在直接运行且无命令行参数时显示）
if (A_Args.Length = 0) {
    ; 创建启动提示窗口
    StartupGui := Gui("+AlwaysOnTop -SysMenu", "剪贴板输入器启动提示")
    StartupGui.SetFont("s10", "Arial")
    StartupGui.Add("Text", "w400 Center", "剪贴板输入器")
    StartupGui.Add("Text", "w400 Center", "制作人：txdt")
    StartupGui.Add("Text", "w400 Center", "本软件完全开源免费，仅供学习使用")
    StartupGui.Add("Text", "w400 Center", "___________________________")
    StartupGui.Add("Text", "w400", "本程序已启动并在后台运行。")
    StartupGui.Add("Text", "w400", "查看系统托盘中的图标进行操作。")
    StartupGui.Add("Text", "w400", "___________________________")
    StartupGui.Add("Text", "w400", "使用方法：")
    StartupGui.Add("Text", "w400", "1. 复制文本到剪贴板")
    StartupGui.Add("Text", "w400", "2. 按 Ctrl+Alt+V 输入内容")
    StartupGui.Add("Text", "w400", "3. 按 Ctrl+. 中断输入")
    StartupGui.Add("Text", "w400", "___________________________")
    StartupGui.Add("Text", "w400", "退出程序：右键点击托盘图标选择退出")
    
    ; 添加确定按钮
    StartupGui.Add("Button", "Default w100", "确定").OnEvent("Click", (*) => StartupGui.Destroy())
    
    ; 显示窗口（非模态）
    StartupGui.Show("Center")
}


; 全局变量用于中断输入
g_Interrupt := false

; 模拟手动输入剪贴板内容，处理自动补全的花括号
^!v:: { ; 热键 Ctrl+Alt+V
    ; 重置中断标志
    g_Interrupt := false
    
    ; 保存原始剪贴板内容
    originalClipboard := ClipboardAll()
    
    ; 获取剪贴板文本
    clipText := A_Clipboard
    if !clipText {
        TrayTip "剪贴板为空！", "剪贴板输入器", "Warning"
        return
    }
    
    ; 设置输入速度（毫秒）
    typingDelay := 10 ; 基本输入速度
    indentDelay := 10 ; 缩进字符额外延迟
    bracketDelay := 80 ; 花括号处理延迟
    
    ; 按行处理文本，保持缩进
    lines := StrSplit(clipText, "`n", "`r")
    for lineIndex, line in lines {
        ; 更频繁地检查中断
        if g_Interrupt
            break
        
        ; 处理每行的缩进（空格和制表符）
        for charIndex, char in StrSplit(line) {
            ; 更频繁地检查中断
            if g_Interrupt
                break 2
            
            ; ==== 核心修改：处理花括号自动补全问题 ====
            if (char == "{") {
                ; 特殊处理左花括号
                SendText "{"          ; 发送左花括号
                Sleep bracketDelay    ; 等待网站自动补全完成
                Send "{Delete}"       ; 使用Delete键删除自动补全的右花括号
                Sleep 20              ; 短暂延迟确保操作完成
            }
            else {
                ; 其他字符正常输入
                SendText char
            }
            ; ========================================
            
            ; 为缩进字符添加额外延迟
            if (char == A_Space || char == A_Tab) {
                Sleep indentDelay
            }
            
            ; 添加随机延迟模拟人手输入
            minDelay := typingDelay - 5
            maxDelay := typingDelay + 0
            randDelay := Random(minDelay, maxDelay)
            Sleep randDelay
            
            ; 添加短暂释放CPU的延迟，确保热键响应
            if (Mod(A_Index, 5) == 0) {
                Sleep 1
            }
        }
        
        ; 如果不是最后一行，输入换行
        if (lineIndex < lines.Length && !g_Interrupt) {
            Send "{Enter}"
            Sleep Random(50, 100)
        }
    }
    
    ; 恢复原始剪贴板内容
    A_Clipboard := originalClipboard

    ; 确保所有修饰键被释放（特别是Alt键）
    Send "{Ctrl up}{Shift up}{Alt up}"
    Sleep 50
    
    ; 显示完成提示
    if g_Interrupt {
        TrayTip "输入已中断", "剪贴板输入器"
    } else {
        TrayTip "输入完成！", "剪贴板输入器"
    }
}

; 中断热键：Ctrl+.
^.::
{
    global g_Interrupt
    g_Interrupt := true
    ; 释放所有修饰键
    Send "{Ctrl up}{Shift up}{Alt up}"
    TrayTip "输入中断", "按 Ctrl+. 停止输入"
}

; 设置托盘图标和菜单
TraySetIcon "shell32.dll", 22
A_TrayMenu.Add "退出", (*) => ExitApp()
A_TrayMenu.Add "关于", (*) => MsgBox("剪贴板输入器 v1.2`n- 修复中断热键响应问题`n- 使用Delete键处理花括号`n- 使用 Ctrl+Alt+V 触发`n- 按 Ctrl+. 中断输入")