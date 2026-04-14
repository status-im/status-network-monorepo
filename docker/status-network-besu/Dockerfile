ARG BESU_PACKAGE_TAG
FROM consensys/linea-besu-package:${BESU_PACKAGE_TAG}

USER root

# Remove stock sequencer plugin (replaced by RLN-enabled version)
RUN rm -f /opt/besu/plugins/linea-sequencer*.jar

# Install custom RLN-enabled sequencer plugin and its dependencies
COPY plugins/ /opt/besu/plugins/

# Install RLN native library
COPY native/librln_bridge.so /opt/besu/lib/native/librln_bridge.so

RUN chown -R besu:besu /opt/besu/plugins/ /opt/besu/lib/native/

USER besu

ENV LD_LIBRARY_PATH="/opt/besu/lib/native:/usr/local/lib:/usr/lib"
ENV JAVA_LIBRARY_PATH="/opt/besu/lib/native"
