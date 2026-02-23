{{/*
Expand the name of the chart.
*/}}
{{- define "status-network.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "status-network.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "status-network.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "status-network.labels" -}}
helm.sh/chart: {{ include "status-network.chart" . }}
{{ include "status-network.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "status-network.selectorLabels" -}}
app.kubernetes.io/name: {{ include "status-network.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "status-network.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "status-network.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Generate a component-specific name
*/}}
{{- define "status-network.componentName" -}}
{{- $top := index . 0 -}}
{{- $component := index . 1 -}}
{{- printf "%s-%s" (include "status-network.fullname" $top) $component | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Component labels
*/}}
{{- define "status-network.componentLabels" -}}
{{- $top := index . 0 -}}
{{- $component := index . 1 -}}
{{ include "status-network.labels" $top }}
app.kubernetes.io/component: {{ $component }}
{{- end }}

{{/*
Component selector labels
*/}}
{{- define "status-network.componentSelectorLabels" -}}
{{- $top := index . 0 -}}
{{- $component := index . 1 -}}
{{ include "status-network.selectorLabels" $top }}
app.kubernetes.io/component: {{ $component }}
{{- end }}

{{/*
Generate the namespace
*/}}
{{- define "status-network.namespace" -}}
{{- .Values.global.namespace | default .Release.Namespace }}
{{- end }}

{{/*
Generate init container to wait for a service
*/}}
{{- define "status-network.waitForService" -}}
- name: wait-for-{{ .name }}
  image: {{ .busyboxImage | default "busybox:latest" }}
  command: ['sh', '-c', 'until nc -z {{ .service }} {{ .port }}; do echo "Waiting for {{ .service }}:{{ .port }}..."; sleep 2; done; echo "{{ .service }}:{{ .port }} is available"']
{{- end }}

{{/*
Generate init container to wait for a file
*/}}
{{- define "status-network.waitForFile" -}}
- name: wait-for-{{ .name }}
  image: {{ .busyboxImage | default "busybox:latest" }}
  command: ['sh', '-c', 'until [ -f {{ .file }} ]; do echo "Waiting for {{ .file }}..."; sleep 2; done; echo "{{ .file }} exists"']
  volumeMounts:
  - name: {{ .volumeName }}
    mountPath: {{ .mountPath }}
{{- end }}

{{/*
Common pod annotations
*/}}
{{- define "status-network.podAnnotations" -}}
checksum/config: {{ include (print $.Template.BasePath "/configmaps/l2-configs.yaml") . | sha256sum }}
{{- end }}

{{/*
Image pull secrets
*/}}
{{- define "status-network.imagePullSecrets" -}}
{{- if .Values.global.imagePullSecrets }}
imagePullSecrets:
{{- range .Values.global.imagePullSecrets }}
  - name: {{ . }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Node selector
*/}}
{{- define "status-network.nodeSelector" -}}
{{- if .Values.nodeSelector }}
nodeSelector:
{{ toYaml .Values.nodeSelector | indent 2 }}
{{- end }}
{{- end }}

{{/*
Tolerations
*/}}
{{- define "status-network.tolerations" -}}
{{- if .Values.tolerations }}
tolerations:
{{ toYaml .Values.tolerations | indent 2 }}
{{- end }}
{{- end }}

{{/*
Affinity
*/}}
{{- define "status-network.affinity" -}}
{{- if .Values.affinity }}
affinity:
{{ toYaml .Values.affinity | indent 2 }}
{{- end }}
{{- end }}

{{/*
Resolve the L1 RPC endpoint.
Uses external endpoint if set, otherwise the internal l1-el-node service.
*/}}
{{- define "status-network.l1RpcEndpoint" -}}
{{- if .Values.network.l1RpcEndpoint -}}
{{ .Values.network.l1RpcEndpoint }}
{{- else -}}
http://l1-el-node:{{ .Values.l1.elNode.ports.http }}
{{- end -}}
{{- end }}

{{/*
Check if L1 network mode is "hoodi"
*/}}
{{- define "status-network.isHoodi" -}}
{{- eq .Values.network.l1Network "hoodi" -}}
{{- end }}
