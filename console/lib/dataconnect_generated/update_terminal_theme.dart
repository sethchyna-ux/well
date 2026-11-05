part of 'generated.dart';

class UpdateTerminalThemeVariablesBuilder {
  String id;
  Optional<double> _opacity = Optional.optional(nativeFromJson, nativeToJson);

  final FirebaseDataConnect _dataConnect;  UpdateTerminalThemeVariablesBuilder opacity(double? t) {
   _opacity.value = t;
   return this;
  }

  UpdateTerminalThemeVariablesBuilder(this._dataConnect, {required  this.id,});
  Deserializer<UpdateTerminalThemeData> dataDeserializer = (dynamic json)  => UpdateTerminalThemeData.fromJson(jsonDecode(json));
  Serializer<UpdateTerminalThemeVariables> varsSerializer = (UpdateTerminalThemeVariables vars) => jsonEncode(vars.toJson());
  Future<OperationResult<UpdateTerminalThemeData, UpdateTerminalThemeVariables>> execute() {
    return ref().execute();
  }

  MutationRef<UpdateTerminalThemeData, UpdateTerminalThemeVariables> ref() {
    UpdateTerminalThemeVariables vars= UpdateTerminalThemeVariables(id: id,opacity: _opacity,);
    return _dataConnect.mutation("UpdateTerminalTheme", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class UpdateTerminalThemeTerminalThemeUpdate {
  final String id;
  UpdateTerminalThemeTerminalThemeUpdate.fromJson(dynamic json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final UpdateTerminalThemeTerminalThemeUpdate otherTyped = other as UpdateTerminalThemeTerminalThemeUpdate;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  UpdateTerminalThemeTerminalThemeUpdate({
    required this.id,
  });
}

@immutable
class UpdateTerminalThemeData {
  final UpdateTerminalThemeTerminalThemeUpdate? terminalTheme_update;
  UpdateTerminalThemeData.fromJson(dynamic json):
  
  terminalTheme_update = json['terminalTheme_update'] == null ? null : UpdateTerminalThemeTerminalThemeUpdate.fromJson(json['terminalTheme_update']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final UpdateTerminalThemeData otherTyped = other as UpdateTerminalThemeData;
    return terminalTheme_update == otherTyped.terminalTheme_update;
    
  }
  @override
  int get hashCode => terminalTheme_update.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    if (terminalTheme_update != null) {
      json['terminalTheme_update'] = terminalTheme_update!.toJson();
    }
    return json;
  }

  UpdateTerminalThemeData({
    this.terminalTheme_update,
  });
}

@immutable
class UpdateTerminalThemeVariables {
  final String id;
  late final Optional<double>opacity;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  UpdateTerminalThemeVariables.fromJson(Map<String, dynamic> json):
  
  id = nativeFromJson<String>(json['id']) {
  
  
  
    opacity = Optional.optional(nativeFromJson, nativeToJson);
    opacity.value = json['opacity'] == null ? null : nativeFromJson<double>(json['opacity']);
  
  }
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final UpdateTerminalThemeVariables otherTyped = other as UpdateTerminalThemeVariables;
    return id == otherTyped.id && 
    opacity == otherTyped.opacity;
    
  }
  @override
  int get hashCode => Object.hashAll([id.hashCode, opacity.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    if(opacity.state == OptionalState.set) {
      json['opacity'] = opacity.toJson();
    }
    return json;
  }

  UpdateTerminalThemeVariables({
    required this.id,
    required this.opacity,
  });
}

