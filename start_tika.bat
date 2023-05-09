ECHO OFF
CLS

SET ROOT_FOLDER="F:\wks-doka-one\doka.one\target\debug"

echo *****************************
echo ***** TIKA SERVER *******
echo *****************************
REM -Dlog4j.configurationFile=file:///D:/test_install/doka.one/service-definitions/log4j.xml
REM java    -jar F:\wks-poc\tika\tika-server-standard-2.2.0.jar --config=D:/test_install/doka.one/service-definitions/config.xml
REM OK java  -jar F:\wks-poc\tika\tika-server-standard-2.2.0.jar --port 40010 -c D:\test_install\doka.one\service-definitions\config.xml
java  -jar D:/test_install/doka.one/bin/tika-server/tika-server-standard-2.2.0.jar -c D:/test_install/doka.one/doka-configs/test_1/tika-server/config/tika-config.xml
