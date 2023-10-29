ECHO OFF
CLS

REM define a environment variable DOKA_PRJ_FOLDER : C:\Users\denis\wks-doka-one
SET ROOT_FOLDER="%DOKA_PRJ_FOLDER%\doka.one\target\debug"

echo *************************
echo ***** KEY MANAGER *******
echo *************************
start "key-manager"  %ROOT_FOLDER%\key-manager.exe

echo **************************
echo ***** SESSION MANAGER ****
echo **************************
start "session-manager" %ROOT_FOLDER%\session-manager.exe

echo **************************
echo ***** ADMIN SERVER *******
echo **************************
start "admin-server" %ROOT_FOLDER%\admin-server.exe

echo *****************************
echo ***** DOCUMENT SERVER *******
echo *****************************
start "document-server" %ROOT_FOLDER%\document-server.exe

echo *****************************
echo ***** FILE SERVER *******
echo *****************************
start "file-server" %ROOT_FOLDER%\file-server.exe

echo *****************************
echo ***** TIKA SERVER *******
echo *****************************
start "tika-server" java -jar %DOKA_PRJ_FOLDER%\tika\tika-server-standard-2.2.0.jar --port 40010

