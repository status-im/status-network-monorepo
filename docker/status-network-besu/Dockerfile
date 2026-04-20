ARG BESU_PACKAGE_TAG
FROM consensys/linea-besu-package:${BESU_PACKAGE_TAG}

USER root

# Remove stock Linea plugins that we replace with versions built from this repo
# (sequencer + tracer + their bundled arithmetization) and the shomei plugin
# that we upgrade. Keeping the stock jars alongside ours causes classpath
# conflicts (NoSuchMethodError on AbstractLineaOptionsPlugin).
RUN rm -f /opt/besu/plugins/linea-sequencer*.jar \
          /opt/besu/plugins/linea-tracer*.jar \
          /opt/besu/plugins/arithmetization-*.jar \
          /opt/besu/plugins/besu-shomei-plugin-*.jar

# Install custom RLN-enabled sequencer + tracer plugins, upgraded shomei, and deps
COPY plugins/ /opt/besu/plugins/

# Install RLN native library
COPY native/librln_bridge.so /opt/besu/lib/native/librln_bridge.so

RUN chown -R besu:besu /opt/besu/plugins/ /opt/besu/lib/native/

USER besu

ENV LD_LIBRARY_PATH="/opt/besu/lib/native:/usr/local/lib:/usr/lib"
ENV JAVA_LIBRARY_PATH="/opt/besu/lib/native"
