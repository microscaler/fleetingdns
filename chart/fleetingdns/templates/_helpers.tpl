{{- define "fleetingdns.namespace" -}}
{{- .Values.namespace | default .Release.Namespace -}}
{{- end -}}

{{- define "fleetingdns.commonLabels" -}}
app.kubernetes.io/part-of: fleetingdns
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end -}}
