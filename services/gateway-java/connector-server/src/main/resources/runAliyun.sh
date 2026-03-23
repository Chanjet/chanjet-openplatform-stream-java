#! /bin/bash

CurrentDir="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

cd ${CurrentDir}



SETENV_DIR=${CurrentDir%/*}'/service'
if [ -x $SETENV_DIR/setenv.sh ];then
. $SETENV_DIR/setenv.sh
fi


eval exec java $JAVA_OPTS -Dfile.encoding=UTF-8 -Duser.country=US -Duser.language=en -Dspring.profiles.active=${ConfigType} -Xms2048m -Xmx2048m -Xmn256m -Xss256k -XX:+UseCompressedOops -XX:+UseG1GC -XX:MaxGCPauseMillis=100 -XX:+UseCompressedClassPointers -jar `pwd`/connector-server.jar
