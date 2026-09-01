part of 'generated.dart';

class GetShellConfigVariablesBuilder {
  String id;

  final FirebaseDataConnect _dataConnect;
  GetShellConfigVariablesBuilder(this._dataConnect, {required  this.id,});
  Deserializer<GetShellConfigData> dataDeserializer = (dynamic json)  => GetShellConfigData.fromJson(jsonDecode(json));
  Serializer<GetShellConfigVariables> varsSerializer = (GetShellConfigVariables vars) => jsonEncode(vars.toJson());
  Future<QueryResult<GetShellConfigData, GetShellConfigVariables>> execute({QueryFetchPolicy fetchPolicy = QueryFetchPolicy.preferCache}) {
    return ref().execute(fetchPolicy: fetchPolicy);
  }

  QueryRef<GetShellConfigData, GetShellConfigVariables> ref() {
    GetShellConfigVariables vars= GetShellConfigVariables(id: id,);
    return _dataConnect.query("GetShellConfig", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class GetShellConfigShellConfig {
  final String promptTemplate;
  final String aliasMapping;
  GetShellConfigShellConfig.fromJson(dynamic json):
  
  promptTemplate = nativeFromJson<String>(json['promptTemplate']),
  aliasMapping = nativeFromJson<String>(json['aliasMapping']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetShellConfigShellConfig otherTyped = other as GetShellConfigShellConfig;
    return promptTemplate == otherTyped.promptTemplate && 
    aliasMapping == otherTyped.aliasMapping;
    
  }
  @override
  int get hashCode => Object.hashAll([promptTemplate.hashCode, aliasMapping.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['promptTemplate'] = nativeToJson<String>(promptTemplate);
    json['aliasMapping'] = nativeToJson<String>(aliasMapping);
    return json;
  }

  GetShellConfigShellConfig({
    required this.promptTemplate,
    required this.aliasMapping,
  });
}

@immutable
class GetShellConfigData {
  final GetShellConfigShellConfig? shellConfig;
  GetShellConfigData.fromJson(dynamic json):
  
  shellConfig = json['shellConfig'] == null ? null : GetShellConfigShellConfig.fromJson(json['shellConfig']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetShellConfigData otherTyped = other as GetShellConfigData;
    return shellConfig == otherTyped.shellConfig;
    
  }
  @override
  int get hashCode => shellConfig.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    if (shellConfig != null) {
      json['shellConfig'] = shellConfig!.toJson();
    }
    return json;
  }

  GetShellConfigData({
    this.shellConfig,
  });
}

@immutable
class GetShellConfigVariables {
  final String id;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  GetShellConfigVariables.fromJson(Map<String, dynamic> json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetShellConfigVariables otherTyped = other as GetShellConfigVariables;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  GetShellConfigVariables({
    required this.id,
  });
}

