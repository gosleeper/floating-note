Set fso = CreateObject("Scripting.FileSystemObject")
base = fso.GetParentFolderName(WScript.ScriptFullName)
Set sh = CreateObject("WScript.Shell")
sh.CurrentDirectory = base
sh.Run """" & base & "\.venv\Scripts\pythonw.exe"" """ & base & "\app.py""", 0, False