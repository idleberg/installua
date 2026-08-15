-- §13 / §15.2: does the editor catch an invalid escape before the compiler does?
local a = "C:\Program Files\app.exe"  -- \P and \a : \a IS valid (bell), \P is not
local b = "C:\\Program Files\\app.exe"
local c = [[C:\Program Files\app.exe]]
local d = "tab\there"
return a, b, c, d
