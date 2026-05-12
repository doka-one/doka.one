ECHO OFF
CLS

REM Path to the built debug binaries (script-relative, falls back to parent project)
SET "ROOT_FOLDER=C:\Users\gcres\Projects\wks-doka-one\doka.one\target\debug"
SET "TIKA_JAR=C:\Users\gcres\Projects\wks-doka-one\tika\tika-server-standard-2.2.0.jar"
SET "JAVA_EXE=C:\Users\gcres\.jdks\corretto-23.0.2\bin\java.exe"
SET "CLUSTER_PROFILE=test_9"

echo *************************
echo ***** KEY MANAGER *******
echo *************************
start "key-manager"  "%ROOT_FOLDER%\key-manager.exe" --cluster-profile %CLUSTER_PROFILE%

echo **************************
echo ***** SESSION MANAGER ****
echo **************************
start "session-manager" "%ROOT_FOLDER%\session-manager.exe" --cluster-profile %CLUSTER_PROFILE%

echo **************************
echo ***** ADMIN SERVER *******
echo **************************
start "admin-server" "%ROOT_FOLDER%\admin-server.exe" --cluster-profile %CLUSTER_PROFILE%

echo *****************************
echo ***** DOCUMENT SERVER *******
echo *****************************
start "document-server" "%ROOT_FOLDER%\document-server.exe" --cluster-profile %CLUSTER_PROFILE%

echo *****************************
echo ***** FILE SERVER *******
echo *****************************
start "file-server" "%ROOT_FOLDER%\file-server.exe" --cluster-profile %CLUSTER_PROFILE%

echo *****************************
echo ***** TIKA SERVER *******
echo *****************************
start "tika-server" "%JAVA_EXE%" -jar "%TIKA_JAR%" --port 40010

